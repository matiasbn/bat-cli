use colored::Colorize;
use error_stack::ResultExt;
use std::{error::Error, fmt};
use tokio::task::JoinSet;

use crate::batbelt::evm::metadata::bat_metadata::{EvmBatMetadata, MiroFrameRef};
use crate::batbelt::miro::frame::{
    MiroFrame, MIRO_BOARD_COLUMNS, MIRO_FRAME_HEIGHT, MIRO_FRAME_WIDTH, MIRO_INITIAL_X,
    MIRO_INITIAL_Y,
};
use crate::batbelt::miro::MiroConfig;

#[derive(Debug)]
pub struct EvmMiroError;

impl fmt::Display for EvmMiroError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EvmMiro error")
    }
}

impl Error for EvmMiroError {}

pub type EvmMiroResult<T> = error_stack::Result<T, EvmMiroError>;

/// Result of a single frame deployment.
struct FrameDeployResult {
    entry_point_name: String,
    frame_id: String,
    frame_url: String,
}

/// Deploy code-overhaul frames for all EVM entry points (parallel).
pub async fn deploy_co_frames() -> EvmMiroResult<()> {
    MiroConfig::check_miro_enabled().change_context(EvmMiroError)?;

    let evm_metadata = EvmBatMetadata::read_metadata().change_context(EvmMiroError)?;
    let ep_names: Vec<String> = evm_metadata
        .entry_points
        .iter()
        .map(|ep| ep.name.clone())
        .collect();

    // Determine which frames need deploying
    let mut to_deploy: Vec<(String, usize)> = Vec::new();
    for (idx, ep_name) in ep_names.iter().enumerate() {
        if let Some(existing) = evm_metadata.get_miro_frame_by_ep_name(ep_name) {
            match MiroFrame::new_from_item_id(&existing.frame_id).await {
                Ok(_) => {
                    println!(
                        "  {} already deployed: {}",
                        ep_name.green(),
                        existing.frame_url
                    );
                    continue;
                }
                Err(_) => {
                    println!("  {} stale frame, redeploying...", ep_name.bright_yellow());
                }
            }
        }
        to_deploy.push((ep_name.clone(), idx));
    }

    if to_deploy.is_empty() {
        println!("  All {} frames already deployed", ep_names.len());
        return Ok(());
    }

    println!(
        "Deploying {} frames to Miro (parallel)...",
        to_deploy.len()
    );

    // Launch all deployments in parallel
    let mut join_set = JoinSet::new();
    for (ep_name, idx) in to_deploy {
        join_set.spawn(async move {
            let result = deploy_single_frame(&ep_name, idx).await;
            (ep_name, result)
        });
    }

    // Collect results
    let mut deployed_frames: Vec<FrameDeployResult> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    while let Some(result) = join_set.join_next().await {
        match result {
            Ok((ep_name, Ok(miro_frame))) => {
                let frame_url = MiroFrame::get_frame_url_by_frame_id(&miro_frame.item_id)
                    .change_context(EvmMiroError)?;
                println!("  {} deployed: {}", ep_name.green(), frame_url);
                deployed_frames.push(FrameDeployResult {
                    entry_point_name: ep_name,
                    frame_id: miro_frame.item_id,
                    frame_url,
                });
            }
            Ok((ep_name, Err(_))) => {
                errors.push(ep_name.clone());
                println!("  {} failed to deploy", ep_name.red());
            }
            Err(e) => {
                errors.push(format!("task panic: {}", e));
            }
        }
    }

    // Write all metadata at once (single atomic write)
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

/// Deploy a single Miro frame for an entry point at the given grid index.
async fn deploy_single_frame(entry_point_name: &str, index: usize) -> EvmMiroResult<MiroFrame> {
    let frame_name = format!("co: {}", entry_point_name);

    let mut miro_frame = MiroFrame::new(&frame_name, MIRO_FRAME_HEIGHT, MIRO_FRAME_WIDTH, 0, 0);
    miro_frame.deploy().await.change_context(EvmMiroError)?;

    let x_modifier = index as i64 % MIRO_BOARD_COLUMNS;
    let y_modifier = index as i64 / MIRO_BOARD_COLUMNS;
    let x_position = MIRO_INITIAL_X + (MIRO_FRAME_WIDTH as i64 + 200) * x_modifier;
    let y_position = MIRO_INITIAL_Y + (MIRO_FRAME_HEIGHT as i64 + 400) * y_modifier;

    miro_frame
        .update_position(x_position, y_position)
        .await
        .change_context(EvmMiroError)?;

    Ok(miro_frame)
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
