//! A single HTTP client for the Miro REST API, with rate limiting and retries.
//!
//! The older helpers scattered across this module each build their own
//! `reqwest::Client`, `.unwrap()` on failures and ignore rate limiting. The
//! automatic deployment issues hundreds of calls in a row, so it needs:
//!
//! - one shared connection pool,
//! - a credit budget that matches Miro's published tiers (Level 2 = 100 credits,
//!   global budget 100_000 credits/minute per user+app),
//! - bounded concurrency,
//! - retries with backoff on `429` and `5xx`, honouring `X-RateLimit-Reset`.

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use error_stack::{IntoReport, Report, Result, ResultExt};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use reqwest::{multipart, StatusCode};
use serde_json::{json, Value};
use tokio::sync::{Mutex, Semaphore};

use crate::batbelt::miro::{MiroConfig, MiroError};

/// Credit cost of a Level 2 endpoint (frames, images, connectors, items).
pub const LEVEL_2_CREDITS: u32 = 100;
/// Credit cost of a Level 1 endpoint (reads).
pub const LEVEL_1_CREDITS: u32 = 50;
/// Miro's global budget, per user and application.
const CREDITS_PER_MINUTE: u32 = 100_000;
/// How many requests we allow in flight at once.
const MAX_CONCURRENT_REQUESTS: usize = 6;
const MAX_ATTEMPTS: u32 = 5;

/// Token bucket over Miro's credit budget.
#[derive(Debug)]
struct CreditBudget {
    window_start: Instant,
    spent: u32,
}

/// Shared, cheap to clone.
#[derive(Clone)]
pub struct MiroClient {
    http: reqwest::Client,
    access_token: String,
    board_id: String,
    board_url: String,
    budget: Arc<Mutex<CreditBudget>>,
    permits: Arc<Semaphore>,
}

/// A frame as returned by the board.
#[derive(Debug, Clone)]
pub struct BoardFrame {
    pub id: String,
    pub title: String,
    /// Center of the frame, in board coordinates.
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl BoardFrame {
    pub fn bottom(&self) -> f64 {
        self.y + self.height / 2.0
    }
    pub fn left(&self) -> f64 {
        self.x - self.width / 2.0
    }
}

/// Where a connector attaches to an item: a relative offset in percent, where
/// `(0%, 0%)` is the item's top-left corner.
#[derive(Debug, Clone, Copy)]
pub struct RelativeAnchor {
    pub x_fraction: f64,
    pub y_fraction: f64,
}

impl RelativeAnchor {
    pub fn new(x_fraction: f64, y_fraction: f64) -> Self {
        Self {
            x_fraction: x_fraction.clamp(0.0, 1.0),
            y_fraction: y_fraction.clamp(0.0, 1.0),
        }
    }

    fn to_json(self) -> Value {
        json!({
            "x": format!("{:.2}%", self.x_fraction * 100.0),
            "y": format!("{:.2}%", self.y_fraction * 100.0),
        })
    }
}

/// Style of one connector.
#[derive(Debug, Clone)]
pub struct ConnectorStyle {
    pub stroke_color: String,
    pub stroke_width: String,
    pub dashed: bool,
    pub caption: Option<String>,
    /// Which end of the connector carries the arrow head, if any.
    pub arrow: ArrowEnd,
}

/// Where a connector's arrow head goes.
///
/// [`ArrowEnd::None`] exists for the middle of a chained edge: an edge routed
/// through invisible bend points is several connectors that must read as one
/// line, so only the last of them may carry a head. Without this, the others
/// draw an arrow pointing at a bend point nobody can see.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ArrowEnd {
    /// On `startItem`. The graph is built caller → callee, but the arrow reads
    /// better pointing the other way: it lands on the exact line that makes the
    /// call, so the picture says "this dependency is used *here*" rather than
    /// restating the direction the layout already shows.
    Start,
    /// On `endItem`.
    End,
    /// Neither end.
    None,
}

impl Default for ConnectorStyle {
    fn default() -> Self {
        Self {
            stroke_color: "#2d9bf0".to_string(),
            stroke_width: "3".to_string(),
            dashed: false,
            caption: None,
            arrow: ArrowEnd::Start,
        }
    }
}

impl MiroClient {
    pub fn new() -> Result<Self, MiroError> {
        let MiroConfig {
            access_token,
            board_id,
            board_url,
        } = MiroConfig::new()?;
        Ok(Self {
            http: reqwest::Client::new(),
            access_token,
            board_id,
            board_url,
            budget: Arc::new(Mutex::new(CreditBudget {
                window_start: Instant::now(),
                spent: 0,
            })),
            permits: Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS)),
        })
    }

    /// Build a client after making sure the globally stored token is still
    /// valid, refreshing it when the app issues expiring tokens.
    pub async fn new_refreshed() -> Result<Self, MiroError> {
        crate::batbelt::miro::auth::refresh_if_needed().await?;
        Self::new()
    }

    pub fn board_url(&self) -> &str {
        &self.board_url
    }

    pub fn frame_url(&self, frame_id: &str) -> String {
        format!("{}/?moveToWidget={}", self.board_url.trim_end_matches('/'), frame_id)
    }

    fn endpoint(&self, path: &str) -> String {
        format!("https://api.miro.com/v2/boards/{}/{}", self.board_id, path)
    }

    /// Wait until `credits` fit in the current minute's budget.
    async fn reserve(&self, credits: u32) {
        loop {
            let wait = {
                let mut budget = self.budget.lock().await;
                if budget.window_start.elapsed() >= Duration::from_secs(60) {
                    budget.window_start = Instant::now();
                    budget.spent = 0;
                }
                if budget.spent + credits <= CREDITS_PER_MINUTE {
                    budget.spent += credits;
                    return;
                }
                Duration::from_secs(60).saturating_sub(budget.window_start.elapsed())
                    + Duration::from_millis(50)
            };
            log::warn!("Miro credit budget exhausted, waiting {:?}", wait);
            tokio::time::sleep(wait).await;
        }
    }

    /// Run a request with rate limiting, retries and error reporting.
    ///
    /// `make` is a closure rather than a prepared request because a multipart
    /// body cannot be cloned, so a retry has to rebuild it from scratch.
    async fn execute<F>(&self, credits: u32, label: &str, make: F) -> Result<Value, MiroError>
    where
        F: Fn(&reqwest::Client) -> reqwest::RequestBuilder,
    {
        let _permit = self
            .permits
            .acquire()
            .await
            .into_report()
            .change_context(MiroError)?;

        let mut attempt = 0;
        loop {
            attempt += 1;
            self.reserve(credits).await;

            let response = make(&self.http)
                .header(AUTHORIZATION, format!("Bearer {}", self.access_token))
                .send()
                .await;

            let response = match response {
                Ok(response) => response,
                Err(error) => {
                    if attempt >= MAX_ATTEMPTS {
                        return Err(Report::new(MiroError)
                            .attach_printable(format!("{label}: transport error: {error}")));
                    }
                    let backoff = Duration::from_millis(400 * 2u64.pow(attempt - 1));
                    log::warn!("{label}: transport error {error}, retrying in {backoff:?}");
                    tokio::time::sleep(backoff).await;
                    continue;
                }
            };

            let status = response.status();
            if status.is_success() {
                let body = response
                    .text()
                    .await
                    .into_report()
                    .change_context(MiroError)?;
                if body.trim().is_empty() {
                    return Ok(Value::Null);
                }
                return serde_json::from_str(&body)
                    .into_report()
                    .change_context(MiroError)
                    .attach_printable_lazy(|| format!("{label}: malformed JSON: {body}"));
            }

            let retryable = status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error();
            if !retryable || attempt >= MAX_ATTEMPTS {
                let body = response.text().await.unwrap_or_default();
                return Err(Report::new(MiroError)
                    .attach_printable(format!("{label}: HTTP {status}: {body}")));
            }

            let wait = retry_after(&response).unwrap_or_else(|| {
                Duration::from_millis(500 * 2u64.pow(attempt - 1))
            });
            log::warn!("{label}: HTTP {status}, retrying in {wait:?} (attempt {attempt})");
            tokio::time::sleep(wait).await;
        }
    }

    /// Every frame currently on the board, following cursor pagination.
    pub async fn list_frames(&self) -> Result<Vec<BoardFrame>, MiroError> {
        let mut frames = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let cursor_for_request = cursor.clone();
            let url = self.endpoint("items");
            let value = self
                .execute(LEVEL_1_CREDITS, "list_frames", move |http| {
                    let mut request = http.get(&url).query(&[("type", "frame"), ("limit", "50")]);
                    if let Some(ref cursor) = cursor_for_request {
                        request = request.query(&[("cursor", cursor.as_str())]);
                    }
                    request
                })
                .await?;

            if let Some(items) = value["data"].as_array() {
                for item in items {
                    frames.push(BoardFrame {
                        id: item["id"].as_str().unwrap_or_default().to_string(),
                        title: item["data"]["title"].as_str().unwrap_or_default().to_string(),
                        x: item["position"]["x"].as_f64().unwrap_or(0.0),
                        y: item["position"]["y"].as_f64().unwrap_or(0.0),
                        width: item["geometry"]["width"].as_f64().unwrap_or(0.0),
                        height: item["geometry"]["height"].as_f64().unwrap_or(0.0),
                    });
                }
            }

            match value["cursor"].as_str() {
                Some(next) if !next.is_empty() => cursor = Some(next.to_string()),
                _ => break,
            }
        }
        Ok(frames)
    }

    /// Create a frame sized to the computed layout. `x`/`y` are the center, in
    /// board coordinates.
    pub async fn create_frame(
        &self,
        title: &str,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Result<String, MiroError> {
        let url = self.endpoint("frames");
        let body = json!({
            "data": { "title": title, "format": "custom", "type": "freeform" },
            "position": { "x": x, "y": y },
            "geometry": { "width": width, "height": height },
        })
        .to_string();

        let value = self
            .execute(LEVEL_2_CREDITS, "create_frame", move |http| {
                http.post(&url)
                    .header(CONTENT_TYPE, "application/json")
                    .body(body.clone())
            })
            .await?;

        Ok(value["id"].as_str().unwrap_or_default().to_string())
    }

    /// Upload a PNG **already positioned, sized and parented**.
    ///
    /// This is the call the previous implementation was missing: without the
    /// `data` part the image lands at the board origin with no parent, which is
    /// why screenshots had to be dragged into place by hand afterwards.
    ///
    /// `x`/`y` are the center of the image relative to the **top-left corner of
    /// the parent frame**. Only `width` is sent: image geometry keeps a fixed
    /// aspect ratio, so setting both width and height is rejected.
    pub async fn create_image_in_frame(
        &self,
        png_path: &str,
        frame_id: &str,
        title: &str,
        x: f64,
        y: f64,
        width: f64,
    ) -> Result<String, MiroError> {
        let bytes = std::fs::read(png_path)
            .into_report()
            .change_context(MiroError)
            .attach_printable_lazy(|| format!("cannot read screenshot {png_path}"))?;

        if bytes.len() > 6 * 1024 * 1024 {
            return Err(Report::new(MiroError).attach_printable(format!(
                "{png_path} is {} MB; Miro rejects images over 6 MB",
                bytes.len() / (1024 * 1024)
            )));
        }

        let file_name = png_path
            .rsplit('/')
            .next()
            .unwrap_or("screenshot.png")
            .to_string();

        let metadata = json!({
            "title": title,
            "position": { "x": x, "y": y },
            "geometry": { "width": width },
            "parent": { "id": frame_id },
        })
        .to_string();

        let url = self.endpoint("images");
        let value = self
            .execute(LEVEL_2_CREDITS, "create_image", move |http| {
                let part = multipart::Part::bytes(bytes.clone())
                    .file_name(file_name.clone())
                    .mime_str("image/png")
                    .expect("image/png is a valid mime type");
                let data = multipart::Part::text(metadata.clone())
                    .mime_str("application/json")
                    .expect("application/json is a valid mime type");
                let form = multipart::Form::new()
                    .part("resource", part)
                    .part("data", data);
                http.post(&url).multipart(form)
            })
            .await?;

        Ok(value["id"].as_str().unwrap_or_default().to_string())
    }

    /// Create an invisible square used as a connector endpoint.
    ///
    /// Miro clips a connector at the boundary of the item it attaches to, so an
    /// anchor placed inside a screenshot is pushed out to that screenshot's
    /// border — the arrow ends up under the token instead of on it. Attaching to
    /// a tiny transparent shape sitting on the token sidesteps that: the border
    /// it gets clipped to *is* the token.
    ///
    /// `x`/`y` are the center, relative to the parent frame's top-left corner.
    pub async fn create_anchor_marker(
        &self,
        frame_id: &str,
        x: f64,
        y: f64,
        size: f64,
    ) -> Result<String, MiroError> {
        let url = self.endpoint("shapes");
        let body = json!({
            "data": { "shape": "rectangle" },
            "style": { "fillOpacity": "0.0", "borderOpacity": "0.0" },
            "position": { "x": x, "y": y },
            "geometry": { "width": size, "height": size },
            "parent": { "id": frame_id },
        })
        .to_string();

        let value = self
            .execute(LEVEL_2_CREDITS, "create_anchor_marker", move |http| {
                http.post(&url)
                    .header(CONTENT_TYPE, "application/json")
                    .body(body.clone())
            })
            .await?;
        Ok(value["id"].as_str().unwrap_or_default().to_string())
    }

    /// Create a card standing in for a function drawn elsewhere on the board.
    ///
    /// The link goes in the shape's `content` as an `<a href>`, because the REST
    /// API rejects Miro's own `linkedTo` field outright — it answers
    /// `Field [linkedTo] is not supported`. An anchor pointing at a
    /// `?moveToWidget=` URL on the same board navigates there instead of opening
    /// a tab, which is what makes this usable at all.
    pub async fn create_link_card(
        &self,
        frame_id: &str,
        title: &str,
        target_url: &str,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Result<String, MiroError> {
        let url = self.endpoint("shapes");
        let body = json!({
            "data": {
                "content": format!(
                    "<p><a href=\"{target_url}\">{title}</a></p><p>ver dependencias →</p>"
                ),
                "shape": "round_rectangle",
            },
            "style": {
                "fillColor": "#fff9b1",
                "borderColor": "#f24726",
                "borderWidth": "2",
                "fontSize": "36",
                "textAlign": "center",
                "textAlignVertical": "middle",
            },
            "position": { "x": x, "y": y },
            "geometry": { "width": width, "height": height },
            "parent": { "id": frame_id },
        })
        .to_string();

        let value = self
            .execute(LEVEL_2_CREDITS, "create_link_card", move |http| {
                http.post(&url)
                    .header(CONTENT_TYPE, "application/json")
                    .body(body.clone())
            })
            .await?;
        Ok(value["id"].as_str().unwrap_or_default().to_string())
    }

    /// Connect two items, anchoring each end at an exact point of the item.
    ///
    /// Note that frames cannot be connector endpoints — the API only accepts
    /// regular items — which is why we always connect image to image.
    pub async fn create_connector(
        &self,
        start_item_id: &str,
        start_anchor: RelativeAnchor,
        end_item_id: &str,
        end_anchor: RelativeAnchor,
        style: ConnectorStyle,
    ) -> Result<String, MiroError> {
        let mut body = json!({
            "startItem": { "id": start_item_id, "position": start_anchor.to_json() },
            "endItem": { "id": end_item_id, "position": end_anchor.to_json() },
            // Always elbowed: orthogonal segments are what makes a call graph
            // readable, and the alternatives were only ever tried to find out
            // whether the shape was what pushed anchors to the item border.
            // It is not — Miro clips a connector at the item boundary whatever
            // the shape.
            "shape": "elbowed",
            "style": {
                "strokeColor": style.stroke_color,
                "strokeWidth": style.stroke_width,
                "strokeStyle": if style.dashed { "dashed" } else { "normal" },
                "startStrokeCap": if style.arrow == ArrowEnd::Start { "stealth" } else { "none" },
                "endStrokeCap": if style.arrow == ArrowEnd::End { "stealth" } else { "none" },
            },
        });
        if let Some(caption) = style.caption {
            body["captions"] = json!([{ "content": caption, "position": "15%" }]);
        }
        let body = body.to_string();

        let url = self.endpoint("connectors");
        let value = self
            .execute(LEVEL_2_CREDITS, "create_connector", move |http| {
                http.post(&url)
                    .header(CONTENT_TYPE, "application/json")
                    .body(body.clone())
            })
            .await?;

        Ok(value["id"].as_str().unwrap_or_default().to_string())
    }
}

/// Prefer `Retry-After`, fall back to `X-RateLimit-Reset` (a UNIX timestamp).
fn retry_after(response: &reqwest::Response) -> Option<Duration> {
    let headers = response.headers();
    if let Some(value) = headers.get("retry-after").and_then(|v| v.to_str().ok()) {
        if let Ok(seconds) = value.parse::<u64>() {
            return Some(Duration::from_secs(seconds));
        }
    }
    let reset = headers
        .get("x-ratelimit-reset")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some(Duration::from_secs(reset.saturating_sub(now).min(120)))
}

#[cfg(test)]
mod client_test {
    use super::*;

    #[test]
    fn test_relative_anchor_is_clamped_and_formatted() {
        let anchor = RelativeAnchor::new(0.6823, 0.374);
        assert_eq!(anchor.to_json()["x"], "68.23%");
        assert_eq!(anchor.to_json()["y"], "37.40%");

        let out_of_range = RelativeAnchor::new(-0.5, 1.9);
        assert_eq!(out_of_range.to_json()["x"], "0.00%");
        assert_eq!(out_of_range.to_json()["y"], "100.00%");
    }
}
