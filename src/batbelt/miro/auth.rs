//! Global Miro authentication: `bat-cli login`.
//!
//! Instead of pasting an OAuth token into every project's `BatAuditor.toml`,
//! the token is obtained once through the browser and stored in the user's
//! config directory, shared by every audit on the machine.
//!
//! The flow is the standard OAuth 2.0 authorization code grant:
//!
//! 1. we start a one-shot HTTP server on `127.0.0.1:9871`,
//! 2. we open `https://miro.com/oauth/authorize` in the browser,
//! 3. the user presses *Accept*, Miro redirects back to localhost with a code,
//! 4. we exchange the code for a token and store it.
//!
//! Miro does not document PKCE support, so the exchange needs a client secret
//! and therefore an app. That app is created once (`bat-cli login --setup`
//! walks through it); after that every teammate only ever clicks *Accept*.
//!
//! Note that a **Developer team cannot be created through the API** — the
//! endpoint that creates teams is Enterprise-only and requires a Company Admin
//! — so that one step stays manual, and `--setup` links straight to it.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use colored::Colorize;
use error_stack::{IntoReport, Report, Result, ResultExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::miro::{app_credentials, MiroError};
use crate::config::{global_config_dir, CONFIG_DIR_ENV};

/// Where Miro sends the browser back. Must match the app's configured redirect
/// URI **exactly**, so the port is fixed rather than picked at random.
const REDIRECT_PORT: u16 = 9871;
const REDIRECT_PATH: &str = "/callback";
const AUTHORIZE_URL: &str = "https://miro.com/oauth/authorize";
const TOKEN_URL: &str = "https://api.miro.com/v1/oauth/token";
const TOKEN_INFO_URL: &str = "https://api.miro.com/v1/oauth-token";
const REVOKE_URL: &str = "https://api.miro.com/v2/oauth/revoke";

/// How long we wait for the user to finish authorizing in the browser.
const AUTHORIZE_TIMEOUT: Duration = Duration::from_secs(300);
/// Refresh a little before the token actually expires.
const REFRESH_SKEW_SECONDS: u64 = 120;

/// Credentials stored globally, outside any audit project.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MiroCredentials {
    /// App identity. Can also come from `BAT_MIRO_CLIENT_ID` / `BAT_MIRO_CLIENT_SECRET`.
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub client_secret: String,
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
    /// Unix seconds. `0` means the app issues non-expiring tokens.
    #[serde(default)]
    pub expires_at: u64,
    #[serde(default)]
    pub team_name: String,
    #[serde(default)]
    pub user_name: String,
}

impl MiroCredentials {
    pub fn load() -> Result<Self, MiroError> {
        let path = Self::path_buf();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)
            .into_report()
            .change_context(MiroError)
            .attach_printable_lazy(|| format!("cannot read {}", path.display()))?;
        toml::from_str(&content)
            .into_report()
            .change_context(MiroError)
            .attach_printable_lazy(|| format!("cannot parse {}", path.display()))
    }

    pub fn save(&self) -> Result<(), MiroError> {
        let path = Self::path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .into_report()
                .change_context(MiroError)
                .attach_printable_lazy(|| format!("cannot create {}", parent.display()))?;
        }
        let content = toml::to_string_pretty(self)
            .into_report()
            .change_context(MiroError)?;
        std::fs::write(&path, content)
            .into_report()
            .change_context(MiroError)
            .attach_printable_lazy(|| format!("cannot write {}", path.display()))?;
        Self::restrict_permissions(&path);
        Ok(())
    }

    pub fn path_buf() -> PathBuf {
        global_config_dir().join("miro.toml")
    }

    pub fn path() -> String {
        Self::path_buf().display().to_string()
    }

    /// The file holds a bearer token, so keep it owner-only.
    fn restrict_permissions(path: &Path) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
        #[cfg(not(unix))]
        let _ = path;
    }

    /// Resolve which Miro app to authorize against.
    ///
    /// Order: environment, then what `--setup` stored, then the compile-time
    /// environment, then the shared app in [`app_credentials`].
    ///
    /// The shared app is what makes the zero-paste flow work: when it is set,
    /// no user ever creates a Miro app or pastes anything — `bat-cli login`
    /// opens the consent page, they pick their team and press Accept.
    fn app_credentials(&self) -> (String, String) {
        let shared = app_credentials::shared_app();
        let pick = |from_env: &str, stored: &str, baked: Option<&str>, shared: Option<&str>| {
            if let Ok(value) = std::env::var(from_env) {
                if !value.trim().is_empty() {
                    return value;
                }
            }
            if !stored.trim().is_empty() {
                return stored.to_string();
            }
            if let Some(baked) = baked {
                if !baked.trim().is_empty() {
                    return baked.to_string();
                }
            }
            shared.unwrap_or_default().to_string()
        };
        (
            pick(
                "BAT_MIRO_CLIENT_ID",
                &self.client_id,
                option_env!("BAT_MIRO_CLIENT_ID"),
                shared.map(|(id, _)| id),
            ),
            pick(
                "BAT_MIRO_CLIENT_SECRET",
                &self.client_secret,
                option_env!("BAT_MIRO_CLIENT_SECRET"),
                shared.map(|(_, secret)| secret),
            ),
        )
    }

    fn is_expired(&self) -> bool {
        self.expires_at != 0 && now_seconds() + REFRESH_SKEW_SECONDS >= self.expires_at
    }
}

/// The token to use, without touching the network.
///
/// Resolution order: environment, then the global credentials. A project's
/// `BatAuditor.toml` still wins over both, and is applied by the caller.
pub fn stored_access_token() -> String {
    if let Ok(token) = std::env::var("MIRO_OAUTH_TOKEN") {
        if !token.trim().is_empty() {
            return token;
        }
    }
    MiroCredentials::load()
        .map(|credentials| credentials.access_token)
        .unwrap_or_default()
}

/// Refresh the stored token if it is about to expire. No-op for apps that issue
/// non-expiring tokens, which is the default when "Expire user authorization
/// token" is left unchecked.
pub async fn refresh_if_needed() -> Result<(), MiroError> {
    let mut credentials = MiroCredentials::load()?;
    if credentials.access_token.is_empty() || !credentials.is_expired() {
        return Ok(());
    }
    if credentials.refresh_token.is_empty() {
        return Err(Report::new(MiroError)
            .attach_printable("the stored Miro token expired and there is no refresh token")
            .attach_printable("run `bat-cli login` again"));
    }

    let (client_id, client_secret) = credentials.app_credentials();
    let response = reqwest::Client::new()
        .post(TOKEN_URL)
        .query(&[
            ("grant_type", "refresh_token"),
            ("client_id", &client_id),
            ("client_secret", &client_secret),
            ("refresh_token", &credentials.refresh_token),
        ])
        .send()
        .await
        .into_report()
        .change_context(MiroError)?;

    let token: Value = parse_token_response(response).await?;
    apply_token_response(&mut credentials, &token);
    credentials.save()?;
    Ok(())
}

/// Run the interactive login.
pub async fn login(setup: bool, force: bool) -> Result<(), MiroError> {
    let mut credentials = MiroCredentials::load()?;

    if setup || credentials.app_credentials().0.is_empty() {
        print_setup_instructions();
        open_in_browser("https://miro.com/app/settings/user-profile/apps");
        credentials.client_id = BatDialoguer::input("Miro app Client ID".to_string())
            .change_context(MiroError)?
            .trim()
            .to_string();
        credentials.client_secret = BatDialoguer::input("Miro app Client secret".to_string())
            .change_context(MiroError)?
            .trim()
            .to_string();
        credentials.save()?;
    }

    if !force && !credentials.access_token.is_empty() && !credentials.is_expired() {
        println!(
            "Already logged in as {} ({}). Use {} to re-authorize.",
            credentials.user_name.green(),
            credentials.team_name.blue(),
            "--force".yellow()
        );
        return Ok(());
    }

    let (client_id, client_secret) = credentials.app_credentials();
    if client_id.is_empty() || client_secret.is_empty() {
        return Err(Report::new(MiroError)
            .attach_printable("no Miro app credentials; run `bat-cli login --setup`"));
    }

    let redirect_uri = format!("http://localhost:{REDIRECT_PORT}{REDIRECT_PATH}");
    let state = random_state();

    // Bind before opening the browser, so the redirect can never arrive first.
    let listener = TcpListener::bind(("127.0.0.1", REDIRECT_PORT))
        .await
        .into_report()
        .change_context(MiroError)
        .attach_printable_lazy(|| {
            format!("cannot listen on port {REDIRECT_PORT}; is another login running?")
        })?;

    let authorize_url = format!(
        "{AUTHORIZE_URL}?response_type=code&client_id={}&redirect_uri={}&state={}",
        percent_encode(&client_id),
        percent_encode(&redirect_uri),
        percent_encode(&state),
    );

    println!("\nOpening Miro in your browser. Press {} there.", "Accept".green());
    println!("If it does not open, paste this URL:\n  {}\n", authorize_url.blue());
    open_in_browser(&authorize_url);

    let code = wait_for_code(listener, &state).await?;

    println!("Authorization received, exchanging it for a token...");
    let response = reqwest::Client::new()
        .post(TOKEN_URL)
        .query(&[
            ("grant_type", "authorization_code"),
            ("client_id", &client_id),
            ("client_secret", &client_secret),
            ("code", &code),
            ("redirect_uri", &redirect_uri),
        ])
        .send()
        .await
        .into_report()
        .change_context(MiroError)?;

    let token = parse_token_response(response).await?;
    apply_token_response(&mut credentials, &token);

    let info = fetch_token_info(&credentials.access_token).await?;
    credentials.team_name = info["team"]["name"].as_str().unwrap_or_default().to_string();
    credentials.user_name = info["user"]["name"].as_str().unwrap_or_default().to_string();
    credentials.save()?;

    println!(
        "\n{} logged in as {} on team {}",
        "✓".green(),
        credentials.user_name.green(),
        credentials.team_name.blue()
    );
    println!("  token stored in {}", MiroCredentials::path());
    if credentials.expires_at == 0 {
        println!("  the token does not expire");
    } else {
        println!("  the token expires in about 60 minutes and refreshes automatically");
    }
    println!(
        "\nProjects now pick the token up automatically; leaving {} empty in\nBatAuditor.toml is enough.",
        "miro_oauth_access_token".yellow()
    );
    Ok(())
}

/// Print who the stored token belongs to, straight from Miro.
pub async fn status() -> Result<(), MiroError> {
    let credentials = MiroCredentials::load()?;
    if credentials.access_token.is_empty() {
        println!("Not logged in. Run {}.", "bat-cli login".yellow());
        return Ok(());
    }
    refresh_if_needed().await?;
    let credentials = MiroCredentials::load()?;
    let info = fetch_token_info(&credentials.access_token).await?;

    println!("Credentials file: {}", MiroCredentials::path());
    println!("User:         {}", info["user"]["name"].as_str().unwrap_or("?").green());
    println!("Team:         {}", info["team"]["name"].as_str().unwrap_or("?").blue());
    println!(
        "Organization: {}",
        info["organization"]["name"].as_str().unwrap_or("-")
    );
    println!("Token type:   {}", info["type"].as_str().unwrap_or("?"));
    if let Some(scopes) = info["scopes"].as_array() {
        let scopes: Vec<&str> = scopes.iter().filter_map(|s| s.as_str()).collect();
        println!("Scopes:       {}", scopes.join(", "));
        if !scopes.iter().any(|s| s.contains("boards:write")) {
            println!(
                "{} the app is missing {}; frames and images cannot be created",
                "warning:".yellow(),
                "boards:write".yellow()
            );
        }
    }
    Ok(())
}

/// Revoke the token on Miro's side and forget it locally.
pub async fn logout() -> Result<(), MiroError> {
    let credentials = MiroCredentials::load()?;
    if credentials.access_token.is_empty() {
        println!("Not logged in.");
        return Ok(());
    }
    let (client_id, _) = credentials.app_credentials();
    let response = reqwest::Client::new()
        .post(REVOKE_URL)
        .query(&[
            ("client_id", client_id.as_str()),
            ("token", credentials.access_token.as_str()),
        ])
        .send()
        .await;
    match response {
        Ok(response) if response.status().is_success() => println!("Token revoked on Miro."),
        Ok(response) => println!(
            "{} Miro answered {} when revoking; clearing the local copy anyway",
            "warning:".yellow(),
            response.status()
        ),
        Err(error) => println!(
            "{} could not reach Miro to revoke ({error}); clearing the local copy anyway",
            "warning:".yellow()
        ),
    }

    // Keep the app credentials so a later `bat-cli login` is one click.
    let cleared = MiroCredentials {
        client_id: credentials.client_id,
        client_secret: credentials.client_secret,
        ..Default::default()
    };
    cleared.save()?;
    println!("Logged out.");
    Ok(())
}

fn print_setup_instructions() {
    let redirect_uri = format!("http://localhost:{REDIRECT_PORT}{REDIRECT_PATH}");
    println!("\n{}\n", "One-time Miro app setup".bold());
    println!(
        "There is no Miro app to authorize against yet, so there is nothing to send\nyou to a consent page for. Creating one takes a minute, and it is the {}\ntime you will see this screen — afterwards {} only opens the browser.\n",
        "only".bold(),
        "bat-cli login".green()
    );
    println!(
        "  1. Your apps page is opening in the browser:\n     {}",
        "https://miro.com/app/settings/user-profile/apps".blue()
    );
    println!("  2. Click {}. If you have no Developer team yet, Miro asks you", "+ Create new app".bold());
    println!("     to create one first: tick the terms checkbox and press \"Create team\".");
    println!("     The app is then assigned to that team automatically.");
    println!(
        "  3. {} \"Expire user authorization token\" — unchecked means the token\n     never expires, which is what a CLI wants.",
        "Leave unchecked".yellow()
    );
    println!(
        "  4. Scopes: check {} and {}.",
        "boards:read".yellow(),
        "boards:write".yellow()
    );
    println!("  5. Redirect URI for OAuth2.0: paste exactly\n     {}", redirect_uri.green());
    println!("  6. Install the app on the team that {} the boards you audit,", "owns".bold());
    println!("     which may not be the Developer team. Otherwise the API answers");
    println!("     404 for those boards.");
    println!(
        "\nThen copy the app's Client ID and Client secret below. They are stored in\n{} and reused by every project, so this is the last copy-paste.\n",
        MiroCredentials::path()
    );
    println!(
        "{} nobody else has to repeat this. One app serves every user — fill in\n{} and your teammates only ever press Accept.\n",
        "Tip:".bold(),
        "src/batbelt/miro/app_credentials.rs".yellow()
    );
}

/// Accept exactly one redirect and pull the `code` out of it.
async fn wait_for_code(listener: TcpListener, expected_state: &str) -> Result<String, MiroError> {
    let accept = async {
        loop {
            let (mut stream, _) = listener
                .accept()
                .await
                .into_report()
                .change_context(MiroError)?;

            let mut buffer = vec![0u8; 8192];
            let read = stream
                .read(&mut buffer)
                .await
                .into_report()
                .change_context(MiroError)?;
            let request = String::from_utf8_lossy(&buffer[..read]).to_string();

            // "GET /callback?code=...&state=... HTTP/1.1"
            let target = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("")
                .to_string();

            if !target.starts_with(REDIRECT_PATH) {
                // Browsers ask for /favicon.ico; ignore anything else.
                let _ = stream.write_all(http_response("Waiting for Miro...").as_bytes()).await;
                continue;
            }

            let params = query_params(&target);
            if let Some(error) = params.iter().find(|(key, _)| key == "error") {
                let _ = stream
                    .write_all(http_response("Authorization denied. You can close this tab.").as_bytes())
                    .await;
                return Err(Report::new(MiroError)
                    .attach_printable(format!("Miro denied the authorization: {}", error.1)));
            }

            let state = params
                .iter()
                .find(|(key, _)| key == "state")
                .map(|(_, value)| value.clone())
                .unwrap_or_default();
            if state != expected_state {
                let _ = stream
                    .write_all(http_response("Unexpected state. You can close this tab.").as_bytes())
                    .await;
                return Err(Report::new(MiroError)
                    .attach_printable("the OAuth state did not match; aborting"));
            }

            let code = params
                .iter()
                .find(|(key, _)| key == "code")
                .map(|(_, value)| value.clone())
                .unwrap_or_default();
            if code.is_empty() {
                let _ = stream
                    .write_all(http_response("No code received. You can close this tab.").as_bytes())
                    .await;
                return Err(Report::new(MiroError).attach_printable("Miro returned no code"));
            }

            let _ = stream
                .write_all(
                    http_response("bat-cli is now connected to Miro. You can close this tab.")
                        .as_bytes(),
                )
                .await;
            let _ = stream.shutdown().await;
            return Ok(code);
        }
    };

    match tokio::time::timeout(AUTHORIZE_TIMEOUT, accept).await {
        Ok(result) => result,
        Err(_) => Err(Report::new(MiroError)
            .attach_printable("timed out waiting for the browser authorization")),
    }
}

fn http_response(message: &str) -> String {
    let body = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>bat-cli</title></head>\
         <body style=\"font-family:-apple-system,system-ui,sans-serif;background:#282a36;\
         color:#f8f8f2;display:flex;align-items:center;justify-content:center;height:100vh;\
         margin:0\"><div style=\"text-align:center\"><div style=\"font-size:48px\">🦇</div>\
         <p style=\"font-size:18px\">{message}</p></div></body></html>"
    );
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
}

async fn parse_token_response(response: reqwest::Response) -> Result<Value, MiroError> {
    let status = response.status();
    let body = response
        .text()
        .await
        .into_report()
        .change_context(MiroError)?;
    if !status.is_success() {
        return Err(Report::new(MiroError)
            .attach_printable(format!("Miro token endpoint answered {status}: {body}")));
    }
    serde_json::from_str(&body)
        .into_report()
        .change_context(MiroError)
        .attach_printable_lazy(|| format!("malformed token response: {body}"))
}

fn apply_token_response(credentials: &mut MiroCredentials, token: &Value) {
    credentials.access_token = token["access_token"].as_str().unwrap_or_default().to_string();
    credentials.refresh_token = token["refresh_token"]
        .as_str()
        .unwrap_or(&credentials.refresh_token)
        .to_string();
    credentials.expires_at = match token["expires_in"].as_u64() {
        Some(seconds) if seconds > 0 => now_seconds() + seconds,
        _ => 0,
    };
}

async fn fetch_token_info(access_token: &str) -> Result<Value, MiroError> {
    let response = reqwest::Client::new()
        .get(TOKEN_INFO_URL)
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .await
        .into_report()
        .change_context(MiroError)?;
    let status = response.status();
    let body = response
        .text()
        .await
        .into_report()
        .change_context(MiroError)?;
    if !status.is_success() {
        return Err(Report::new(MiroError)
            .attach_printable(format!("cannot read the token info ({status}): {body}")));
    }
    serde_json::from_str(&body)
        .into_report()
        .change_context(MiroError)
}

fn open_in_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let command = Command::new("open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let command = Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let command = Command::new("cmd").args(["/C", "start", "", url]).spawn();

    if let Err(error) = command {
        log::warn!("could not open the browser automatically: {error}");
    }
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn random_state() -> String {
    use rand::distributions::Alphanumeric;
    use rand::Rng;
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(24)
        .map(char::from)
        .collect()
}

/// Percent-encode everything that is not unreserved, which is all we need for
/// a client id, a redirect URI and a state token.
fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(decoded) => {
                        out.push(decoded);
                        index += 3;
                    }
                    Err(_) => {
                        out.push(bytes[index]);
                        index += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            other => {
                out.push(other);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

fn query_params(target: &str) -> Vec<(String, String)> {
    let Some((_, query)) = target.split_once('?') else {
        return Vec::new();
    };
    query
        .split('&')
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            Some((percent_decode(key), percent_decode(value)))
        })
        .collect()
}

#[cfg(test)]
mod auth_test {
    use super::*;

    /// Both halves live in one test on purpose: they mutate process-wide
    /// environment variables, and `cargo test` runs tests in parallel threads.
    #[test]
    fn test_config_dir_and_credentials_round_trip() {
        let _guard = crate::config::CONFIG_ENV_GUARD
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        temp_env(CONFIG_DIR_ENV, Some("/tmp/bat-cli-test"), || {
            assert_eq!(global_config_dir(), PathBuf::from("/tmp/bat-cli-test"));
        });
        temp_env(CONFIG_DIR_ENV, None, || {
            temp_env("XDG_CONFIG_HOME", Some("/tmp/xdg"), || {
                assert_eq!(global_config_dir(), PathBuf::from("/tmp/xdg/bat-cli"));
            });
            // With no XDG override it must land on ~/.config/bat-cli.
            temp_env("XDG_CONFIG_HOME", None, || {
                let home = std::env::var("HOME").unwrap();
                assert_eq!(
                    global_config_dir(),
                    PathBuf::from(home).join(".config").join("bat-cli")
                );
            });
        });

        let directory = std::env::temp_dir().join("bat_cli_credentials_test");
        let _ = std::fs::remove_dir_all(&directory);
        temp_env(CONFIG_DIR_ENV, Some(directory.to_str().unwrap()), || {
            // A missing file must read as empty rather than failing.
            assert!(MiroCredentials::load().unwrap().access_token.is_empty());

            let credentials = MiroCredentials {
                client_id: "id".to_string(),
                access_token: "secret-token".to_string(),
                ..Default::default()
            };
            credentials.save().unwrap();

            let loaded = MiroCredentials::load().unwrap();
            assert_eq!(loaded.access_token, "secret-token");
            assert_eq!(loaded.client_id, "id");

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(MiroCredentials::path_buf())
                    .unwrap()
                    .permissions()
                    .mode();
                assert_eq!(mode & 0o777, 0o600, "the token file must not be readable by others");
            }
        });
        let _ = std::fs::remove_dir_all(&directory);
    }

    /// The env var tests mutate process state, so they run the body inline and
    /// restore the previous value afterwards.
    fn temp_env<F: FnOnce()>(key: &str, value: Option<&str>, body: F) {
        let previous = std::env::var(key).ok();
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
        body();
        match previous {
            Some(previous) => std::env::set_var(key, previous),
            None => std::env::remove_var(key),
        }
    }

    #[test]
    fn test_query_params_are_decoded() {
        let params = query_params("/callback?code=abc%2F123&state=xy%20z");
        assert_eq!(params[0], ("code".to_string(), "abc/123".to_string()));
        assert_eq!(params[1], ("state".to_string(), "xy z".to_string()));
    }

    #[test]
    fn test_redirect_uri_round_trips_through_encoding() {
        let redirect = format!("http://localhost:{REDIRECT_PORT}{REDIRECT_PATH}");
        let encoded = percent_encode(&redirect);
        assert!(!encoded.contains(':'), "the colon must be escaped");
        assert_eq!(percent_decode(&encoded), redirect);
    }

    #[test]
    fn test_expiring_and_non_expiring_tokens() {
        let mut credentials = MiroCredentials::default();

        apply_token_response(
            &mut credentials,
            &serde_json::json!({ "access_token": "t1", "expires_in": 3600 }),
        );
        assert_eq!(credentials.access_token, "t1");
        assert!(credentials.expires_at > now_seconds());
        assert!(!credentials.is_expired());

        // No `expires_in` means the app issues non-expiring tokens.
        apply_token_response(
            &mut credentials,
            &serde_json::json!({ "access_token": "t2" }),
        );
        assert_eq!(credentials.expires_at, 0);
        assert!(!credentials.is_expired());

        // A token already inside the refresh skew counts as expired.
        credentials.expires_at = now_seconds() + REFRESH_SKEW_SECONDS - 1;
        assert!(credentials.is_expired());
    }

    #[test]
    fn test_refresh_token_is_kept_when_the_response_omits_it() {
        let mut credentials = MiroCredentials {
            refresh_token: "keep-me".to_string(),
            ..Default::default()
        };
        apply_token_response(
            &mut credentials,
            &serde_json::json!({ "access_token": "t", "expires_in": 60 }),
        );
        assert_eq!(credentials.refresh_token, "keep-me");
    }
}
