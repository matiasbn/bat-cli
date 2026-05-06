use colored::Colorize;
use error_stack::{Report, ResultExt};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde_json::{json, Value};
use std::collections::{HashSet, VecDeque};
use std::{error::Error, fmt};
use tokio::task::JoinSet;

use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::evm::metadata::bat_metadata::{EvmBatMetadata, MiroFrameRef};
use crate::batbelt::miro::connector::{create_connector, create_connector_with_color};
use crate::batbelt::miro::frame::{
    MiroFrame, MIRO_BOARD_COLUMNS, MIRO_FRAME_HEIGHT, MIRO_FRAME_WIDTH, MIRO_INITIAL_X,
    MIRO_INITIAL_Y,
};
use crate::batbelt::miro::sticky_note::MiroStickyNote;
use crate::batbelt::miro::{MiroColor, MiroConfig};
use crate::batbelt::parser::source_code_parser::{SourceCodeParser, SourceCodeScreenshotOptions};
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
                    images_deployed: false,
                    entry_point_image_id: String::new(),
                    validations_image_id: String::new(),
                    dependency_image_ids: vec![],
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

/// Deploy code-overhaul screenshots for a single EVM entry point into its Miro frame.
///
/// Deploys: entry point screenshot, access control sticky note, validations screenshot,
/// and BFS dependency screenshots with connectors.
pub async fn deploy_co_screenshots(entry_point_name: &str) -> EvmMiroResult<()> {
    MiroConfig::check_miro_enabled().change_context(EvmMiroError)?;

    let evm_metadata = EvmBatMetadata::read_metadata().change_context(EvmMiroError)?;

    // Find frame ref
    let frame_ref = evm_metadata
        .get_miro_frame_by_ep_name(entry_point_name)
        .ok_or_else(|| {
            Report::new(EvmMiroError).attach_printable(format!(
                "No Miro frame found for '{}'. Run miro code-overhaul-frames first.",
                entry_point_name
            ))
        })?
        .clone();

    if frame_ref.images_deployed {
        println!(
            "  Screenshots already deployed for {}",
            entry_point_name.green()
        );
        return Ok(());
    }

    // Get the MiroFrame object
    let co_miro_frame = MiroFrame::new_from_item_id(&frame_ref.frame_id)
        .await
        .change_context(EvmMiroError)?;

    println!(
        "Deploying screenshots for {} to frame {}",
        entry_point_name.green(),
        co_miro_frame.title.green()
    );

    // Look up entry point and function metadata
    let ep = evm_metadata
        .get_entry_point_by_name(entry_point_name)
        .ok_or_else(|| {
            Report::new(EvmMiroError)
                .attach_printable(format!("Entry point '{}' not found", entry_point_name))
        })?
        .clone();

    let contract = evm_metadata
        .get_contract_by_name(&ep.contract_name)
        .ok_or_else(|| {
            Report::new(EvmMiroError)
                .attach_printable(format!("Contract '{}' not found", ep.contract_name))
        })?
        .clone();

    let func = evm_metadata
        .get_function_by_id(&ep.function_metadata_id)
        .ok_or_else(|| {
            Report::new(EvmMiroError).attach_printable(format!(
                "Function '{}' not found",
                ep.function_metadata_id
            ))
        })?
        .clone();

    // 1. Deploy entry point function screenshot at (1680, 260)
    let ep_end_line = if func.end_line > 0 {
        func.end_line
    } else {
        // Fallback: read file and count from body_source
        let content = std::fs::read_to_string(&contract.file_path).unwrap_or_default();
        let total_lines = content.lines().count();
        // Estimate: start line + body lines
        (func.line + 30).min(total_lines)
    };

    let ep_sc = SourceCodeParser::new(
        format!("ep_{}", func.name),
        contract.file_path.clone(),
        func.line,
        ep_end_line,
    );

    let ep_image = ep_sc
        .deploy_screenshot_to_miro_frame(
            co_miro_frame.clone(),
            1680,
            260,
            SourceCodeScreenshotOptions {
                include_path: true,
                offset_to_start_line: true,
                filter_comments: false,
                font_size: Some(20),
                filters: None,
                show_line_number: true,
            },
        )
        .await
        .change_context(EvmMiroError)?;

    let entry_point_image_id = ep_image.item_id.clone();

    // 2. Deploy access control sticky note at (200, 260)
    let access_control_text = {
        let ac_str = ep
            .access_control
            .iter()
            .map(|ac| format!("{:?}", ac))
            .collect::<Vec<_>>()
            .join("<br>");
        let mod_str = if ep.modifiers.is_empty() {
            "None".to_string()
        } else {
            ep.modifiers.join(", ")
        };
        format!(
            "<strong>Access Control</strong><br>{}<br><br><strong>Modifiers:</strong> {}",
            ac_str, mod_str
        )
    };

    let mut access_note = MiroStickyNote::new(
        &access_control_text,
        MiroColor::LightYellow,
        &co_miro_frame.item_id,
        200,
        260,
        374,
        0,
    );
    access_note.deploy().await.change_context(EvmMiroError)?;

    // Connect access control to entry point
    create_connector(&access_note.item_id, &entry_point_image_id, None)
        .await
        .change_context(EvmMiroError)?;

    // 3. Deploy validations screenshot at (4200, 650)
    // Extract require/revert/assert lines from the function body
    let validations_image_id = {
        let file_content =
            std::fs::read_to_string(&contract.file_path).unwrap_or_default();
        let file_lines: Vec<&str> = file_content.lines().collect();

        // Find validation lines (require, revert, assert) within the function range
        let mut validation_lines: Vec<(usize, String)> = Vec::new();
        let start = if func.line > 0 { func.line - 1 } else { 0 };
        let end = ep_end_line.min(file_lines.len());

        for i in start..end {
            let trimmed = file_lines[i].trim();
            if trimmed.starts_with("require(")
                || trimmed.starts_with("require (")
                || trimmed.starts_with("revert ")
                || trimmed.starts_with("revert(")
                || trimmed.starts_with("assert(")
                || trimmed.starts_with("assert (")
                || trimmed.contains("revert ")
                || trimmed.starts_with("if (")
                    && file_lines
                        .get(i + 1)
                        .map_or(false, |next| next.trim().starts_with("revert"))
            {
                validation_lines.push((i + 1, file_lines[i].to_string()));
            }
        }

        if !validation_lines.is_empty() {
            // Build a validation source code block
            let val_content = validation_lines
                .iter()
                .map(|(line_num, content)| format!("L{}: {}", line_num, content.trim()))
                .collect::<Vec<_>>()
                .join("\n");

            // Write temp content for screenshot
            let val_name = format!("validations_{}", func.name);
            let temp_path = format!("/tmp/{}.sol", val_name);
            std::fs::write(&temp_path, &val_content).unwrap_or_default();

            let val_sc = SourceCodeParser::new(val_name, temp_path, 1, validation_lines.len());
            let val_image = val_sc
                .deploy_screenshot_to_miro_frame(
                    co_miro_frame.clone(),
                    4200,
                    650,
                    SourceCodeScreenshotOptions {
                        include_path: false,
                        offset_to_start_line: false,
                        filter_comments: false,
                        font_size: Some(12),
                        filters: None,
                        show_line_number: false,
                    },
                )
                .await
                .change_context(EvmMiroError)?;

            // Connect entry point to validations
            create_connector(&entry_point_image_id, &val_image.item_id, None)
                .await
                .change_context(EvmMiroError)?;

            val_image.item_id
        } else {
            // No validations found, deploy a note instead
            let mut no_val_note = MiroStickyNote::new(
                "<strong>Validations</strong><br>No require/revert/assert found",
                MiroColor::Gray,
                &co_miro_frame.item_id,
                4200,
                650,
                300,
                0,
            );
            no_val_note.deploy().await.change_context(EvmMiroError)?;

            create_connector(&entry_point_image_id, &no_val_note.item_id, None)
                .await
                .change_context(EvmMiroError)?;

            no_val_note.item_id
        }
    };

    // 4. BFS deployment of dependency screenshots
    let mut deployed_function_ids: HashSet<String> = HashSet::new();
    deployed_function_ids.insert(ep.function_metadata_id.clone());

    let mut id_to_image: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    id_to_image.insert(ep.function_metadata_id.clone(), entry_point_image_id.clone());

    let mut bfs_queue: VecDeque<(String, String)> = VecDeque::new();
    bfs_queue.push_back((ep.function_metadata_id.clone(), func.name.clone()));

    let mut dependency_image_ids: Vec<String> = Vec::new();

    const CASCADE_START_X: i64 = 2400;
    const CASCADE_START_Y: i64 = 900;
    const CASCADE_STEP: i64 = 60;

    const DEP_ARROW_COLORS: &[(&str, &str)] = &[
        ("#ff0000", "red"),
        ("#0000ff", "blue"),
        ("#00aa00", "green"),
        ("#ff8800", "orange"),
        ("#aa00ff", "purple"),
        ("#00cccc", "teal"),
        ("#ff00aa", "pink"),
        ("#888800", "olive"),
    ];
    let mut color_index: usize = 0;

    // Helper: get direct deps from function_dependencies
    let direct_deps_of = |function_id: &str| -> Vec<String> {
        evm_metadata
            .function_dependencies
            .iter()
            .find(|fd| fd.function_metadata_id == function_id)
            .map(|fd| fd.callees.clone())
            .unwrap_or_default()
    };

    while let Some((caller_id, caller_name)) = bfs_queue.pop_front() {
        let direct_deps = direct_deps_of(&caller_id);
        let new_deps: Vec<String> = direct_deps
            .into_iter()
            .filter(|id| !deployed_function_ids.contains(id))
            .collect();

        if new_deps.is_empty() {
            continue;
        }

        // Resolve function metadata for each dep
        let mut new_dep_functions: Vec<(
            String,
            String,
            String,
            usize,
            usize,
        )> = Vec::new(); // (metadata_id, name, file_path, line, end_line)

        for dep_id in &new_deps {
            if let Some(dep_func) = evm_metadata.get_function_by_id(dep_id) {
                let dep_contract = evm_metadata
                    .get_contract_by_name(&dep_func.contract_name)
                    .map(|c| c.file_path.clone())
                    .unwrap_or_default();
                let end = if dep_func.end_line > 0 {
                    dep_func.end_line
                } else {
                    dep_func.line + 20
                };
                new_dep_functions.push((
                    dep_func.metadata_id.clone(),
                    dep_func.name.clone(),
                    dep_contract,
                    dep_func.line,
                    end,
                ));
            }
        }

        if new_dep_functions.is_empty() {
            continue;
        }

        let (arrow_hex, arrow_name) = DEP_ARROW_COLORS[color_index % DEP_ARROW_COLORS.len()];
        color_index += 1;

        let prompt = format!(
            "Press Enter to deploy {} dependencies of `{}` (arrow color: {})",
            new_dep_functions.len(),
            caller_name,
            arrow_name
        );
        BatDialoguer::input_with_default(prompt, "".to_string())
            .change_context(EvmMiroError)?;

        for (idx, (dep_id, dep_name, dep_path, dep_line, dep_end)) in
            new_dep_functions.iter().enumerate()
        {
            let dep_sc = SourceCodeParser::new(
                format!("dep_{}", dep_name),
                dep_path.clone(),
                *dep_line,
                *dep_end,
            );

            let cascade_x = CASCADE_START_X + (idx as i64) * CASCADE_STEP;
            let cascade_y = CASCADE_START_Y + (idx as i64) * CASCADE_STEP;

            let dep_image = dep_sc
                .deploy_screenshot_to_miro_frame(
                    co_miro_frame.clone(),
                    cascade_x,
                    cascade_y,
                    SourceCodeScreenshotOptions {
                        include_path: true,
                        offset_to_start_line: true,
                        filter_comments: false,
                        font_size: Some(16),
                        filters: None,
                        show_line_number: true,
                    },
                )
                .await
                .change_context(EvmMiroError)?;

            // Arrow from dep to caller
            if let Some(caller_image_id) = id_to_image.get(&caller_id) {
                create_connector_with_color(
                    &dep_image.item_id,
                    caller_image_id,
                    None,
                    Some(arrow_hex),
                )
                .await
                .change_context(EvmMiroError)?;
            }

            id_to_image.insert(dep_id.clone(), dep_image.item_id.clone());
            deployed_function_ids.insert(dep_id.clone());
            dependency_image_ids.push(dep_image.item_id.clone());

            bfs_queue.push_back((dep_id.clone(), dep_name.clone()));
        }
    }

    // Update metadata with deployed image IDs
    EvmBatMetadata::update_metadata(|metadata| {
        if let Some(frame) = metadata
            .miro
            .frames
            .iter_mut()
            .find(|f| f.entry_point_name == entry_point_name)
        {
            frame.images_deployed = true;
            frame.entry_point_image_id = entry_point_image_id.clone();
            frame.validations_image_id = validations_image_id.clone();
            frame.dependency_image_ids = dependency_image_ids.clone();
        }
    })
    .change_context(EvmMiroError)?;

    // Update CO file with Miro frame URL
    if let Some(ref frame_url) = co_miro_frame.frame_url {
        let co_file_name = format!("{}.md", entry_point_name);
        let co_bat_file = crate::batbelt::path::BatFile::CodeOverhaulStarted {
            file_name: co_file_name,
            program_name: None,
        };
        if let Ok(content) = co_bat_file.read_content(true) {
            let placeholder = "`COMPLETE_WITH_MIRO_FRAME_URL`";
            if content.contains(placeholder) {
                let new_content = content.replace(placeholder, frame_url);
                let _ = co_bat_file.write_content(true, &new_content);
            }
        }
    }

    println!(
        "  Screenshots deployed for {}",
        entry_point_name.green()
    );

    Ok(())
}
