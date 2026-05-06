use colored::Colorize;
use error_stack::{Report, ResultExt};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde_json::{json, Value};
use std::{error::Error, fmt};
use tokio::task::JoinSet;

use crate::batbelt::evm::metadata::bat_metadata::{EvmBatMetadata, MiroFrameRef};
use crate::batbelt::miro::frame::{
    MIRO_BOARD_COLUMNS, MIRO_FRAME_HEIGHT, MIRO_FRAME_WIDTH, MIRO_INITIAL_X, MIRO_INITIAL_Y,
};
use crate::batbelt::miro::MiroConfig;
use crate::config::BatConfig;

#[derive(Debug)]
pub struct EvmMiroError;

impl fmt::Display for EvmMiroError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EvmMiro error")
    }
}

impl Error for EvmMiroError {}

pub type EvmMiroResult<T> = error_stack::Result<T, EvmMiroError>;

/// Pre-read config needed for Miro API calls.
#[derive(Clone)]
struct MiroApiConfig {
    access_token: String,
    board_id: String,
    board_url: String,
}

/// Result of a single frame deployment.
struct FrameDeployResult {
    entry_point_name: String,
    frame_id: String,
    frame_url: String,
}

/// Deploy code-overhaul frames for all EVM entry points (parallel).
pub async fn deploy_co_frames() -> EvmMiroResult<()> {
    MiroConfig::check_miro_enabled().change_context(EvmMiroError)?;

    // Read config ONCE before spawning any tasks
    let api_config = read_miro_config()?;

    let evm_metadata = EvmBatMetadata::read_metadata().change_context(EvmMiroError)?;
    let ep_names: Vec<String> = evm_metadata
        .entry_points
        .iter()
        .map(|ep| ep.name.clone())
        .collect();

    // Determine which frames need deploying (sequential — only checks existing)
    let mut to_deploy: Vec<(String, usize)> = Vec::new();
    for (idx, ep_name) in ep_names.iter().enumerate() {
        if let Some(existing) = evm_metadata.get_miro_frame_by_ep_name(ep_name) {
            // Verify frame still exists (single sequential check)
            if verify_frame_exists(&api_config, &existing.frame_id).await {
                println!(
                    "  {} already deployed: {}",
                    ep_name.green(),
                    existing.frame_url
                );
                continue;
            }
            println!("  {} stale frame, redeploying...", ep_name.bright_yellow());
        }
        to_deploy.push((ep_name.clone(), idx));
    }

    if to_deploy.is_empty() {
        println!("  All {} frames already deployed", ep_names.len());
        return Ok(());
    }

    println!(
        "  Deploying {} frames in parallel...",
        to_deploy.len()
    );

    // Launch all deployments in parallel — each task gets a clone of the config
    let mut join_set = JoinSet::new();
    for (ep_name, idx) in to_deploy {
        let config = api_config.clone();
        join_set.spawn(async move {
            let result = deploy_single_frame(&config, &ep_name, idx).await;
            (ep_name, result)
        });
    }

    // Collect results
    let mut deployed_frames: Vec<FrameDeployResult> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    while let Some(result) = join_set.join_next().await {
        match result {
            Ok((ep_name, Ok((frame_id, frame_url)))) => {
                println!("  {} deployed: {}", ep_name.green(), frame_url);
                deployed_frames.push(FrameDeployResult {
                    entry_point_name: ep_name,
                    frame_id,
                    frame_url,
                });
            }
            Ok((ep_name, Err(e))) => {
                errors.push(ep_name.clone());
                println!("  {} failed: {:?}", ep_name.red(), e);
            }
            Err(e) => {
                errors.push(format!("task panic: {}", e));
            }
        }
    }

    // Write all metadata at once (single atomic write, no race)
    if !deployed_frames.is_empty() {
        EvmBatMetadata::update_metadata(|metadata| {
            for frame in &deployed_frames {
                metadata
                    .miro
                    .frames
                    .retain(|f| f.entry_point_name != frame.entry_point_name);
                metadata.miro.frames.push(MiroFrameRef {
                    entry_point_name: frame.entry_point_name.clone(),
                    frame_id: frame.frame_id.clone(),
                    frame_url: frame.frame_url.clone(),
                });
            }
        })
        .change_context(EvmMiroError)?;
    }

    if !errors.is_empty() {
        println!(
            "  {} {} frames failed to deploy",
            "Warning:".bright_yellow(),
            errors.len()
        );
    }

    Ok(())
}

/// Read Miro config once (access_token, board_id, board_url).
fn read_miro_config() -> EvmMiroResult<MiroApiConfig> {
    let miro_config = MiroConfig::new().change_context(EvmMiroError)?;
    // MiroConfig fields are private, re-read from source
    let bat_config = BatConfig::get_config().change_context(EvmMiroError)?;
    let bat_auditor_config =
        crate::config::BatAuditorConfig::get_config().change_context(EvmMiroError)?;
    let board_url = bat_config.miro_board_url.clone();
    let board_id = extract_board_id(&board_url)?;

    Ok(MiroApiConfig {
        access_token: bat_auditor_config.miro_oauth_access_token,
        board_id,
        board_url,
    })
}

/// Extract board ID from Miro board URL.
fn extract_board_id(board_url: &str) -> EvmMiroResult<String> {
    // URL format: https://miro.com/app/board/BOARD_ID=/
    board_url
        .split("/board/")
        .nth(1)
        .and_then(|s| s.split('/').next())
        .map(|s| s.trim_end_matches('=').to_string() + "=")
        .ok_or_else(|| {
            Report::new(EvmMiroError).attach_printable(format!(
                "Cannot extract board_id from URL: {}",
                board_url
            ))
        })
}

/// Build frame URL from board URL and frame ID.
fn build_frame_url(board_url: &str, frame_id: &str) -> String {
    format!("{}/?moveToWidget={}", board_url, frame_id)
}

/// Verify a frame still exists on the board.
async fn verify_frame_exists(config: &MiroApiConfig, frame_id: &str) -> bool {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!(
            "https://api.miro.com/v2/boards/{}/items/{}",
            config.board_id, frame_id
        ))
        .header(AUTHORIZATION, format!("Bearer {}", config.access_token))
        .send()
        .await;
    matches!(resp, Ok(r) if r.status().is_success())
}

/// Deploy a single frame and return (frame_id, frame_url).
async fn deploy_single_frame(
    config: &MiroApiConfig,
    entry_point_name: &str,
    index: usize,
) -> EvmMiroResult<(String, String)> {
    let frame_name = format!("co: {}", entry_point_name);
    let client = reqwest::Client::new();

    // Create frame at origin
    let response = client
        .post(format!(
            "https://api.miro.com/v2/boards/{}/frames",
            config.board_id
        ))
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, format!("Bearer {}", config.access_token))
        .body(
            json!({
                "data": {
                    "format": "custom",
                    "title": frame_name,
                    "type": "freeform"
                },
                "position": {
                    "origin": "center",
                    "x": 0,
                    "y": 0
                },
                "geometry": {
                    "width": MIRO_FRAME_WIDTH,
                    "height": MIRO_FRAME_HEIGHT
                }
            })
            .to_string(),
        )
        .send()
        .await
        .map_err(|e| {
            Report::new(EvmMiroError).attach_printable(format!("HTTP error creating frame: {}", e))
        })?;

    let body = response.text().await.map_err(|e| {
        Report::new(EvmMiroError).attach_printable(format!("Cannot read response body: {}", e))
    })?;

    let value: Value = serde_json::from_str(&body).map_err(|e| {
        Report::new(EvmMiroError).attach_printable(format!("Cannot parse Miro response: {}", e))
    })?;

    let frame_id = value["id"]
        .as_str()
        .ok_or_else(|| Report::new(EvmMiroError).attach_printable("No 'id' in Miro response"))?
        .to_string();

    // Calculate grid position
    let x_modifier = index as i64 % MIRO_BOARD_COLUMNS;
    let y_modifier = index as i64 / MIRO_BOARD_COLUMNS;
    let x_position = MIRO_INITIAL_X + (MIRO_FRAME_WIDTH as i64 + 200) * x_modifier;
    let y_position = MIRO_INITIAL_Y + (MIRO_FRAME_HEIGHT as i64 + 400) * y_modifier;

    // Update position
    client
        .patch(format!(
            "https://api.miro.com/v2/boards/{}/frames/{}",
            config.board_id, frame_id
        ))
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, format!("Bearer {}", config.access_token))
        .body(
            json!({
                "position": {
                    "x": x_position,
                    "y": y_position,
                    "origin": "center"
                }
            })
            .to_string(),
        )
        .send()
        .await
        .map_err(|e| {
            Report::new(EvmMiroError)
                .attach_printable(format!("HTTP error updating position: {}", e))
        })?;

    let frame_url = build_frame_url(&config.board_url, &frame_id);

    Ok((frame_id, frame_url))
}

/// Get sorted entry point names from EVM metadata (for selection prompts).
pub fn get_entry_point_names() -> EvmMiroResult<Vec<String>> {
    let evm_metadata = EvmBatMetadata::read_metadata().change_context(EvmMiroError)?;
    let mut names: Vec<String> = evm_metadata
        .entry_points
        .iter()
        .map(|ep| ep.name.clone())
        .collect();
    names.sort();
    Ok(names)
}
