use colored::Colorize;
use error_stack::{Report, ResultExt};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde_json::{json, Value};
use std::collections::{HashSet, VecDeque};
use std::{error::Error, fmt};
use tokio::task::JoinSet;

use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::evm::metadata::bat_metadata::{EvmBatMetadata, MiroFrameRef};
use crate::batbelt::miro::connector::create_connector_with_color;
use crate::batbelt::miro::frame::{
    MiroFrame, MIRO_BOARD_COLUMNS, MIRO_FRAME_HEIGHT, MIRO_FRAME_WIDTH, MIRO_INITIAL_X,
    MIRO_INITIAL_Y,
};
use crate::batbelt::miro::MiroConfig;
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

/// Find the closing brace of a Solidity function starting at `start_line` (1-based).
/// Uses brace-depth counting to handle nested blocks.
fn find_function_end_line(file_path: &str, start_line: usize) -> usize {
    let content = std::fs::read_to_string(file_path).unwrap_or_default();
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    let start_idx = if start_line > 0 { start_line - 1 } else { 0 };

    let mut depth: i32 = 0;
    let mut found_open = false;

    for i in start_idx..total {
        for ch in lines[i].chars() {
            if ch == '{' {
                depth += 1;
                found_open = true;
            } else if ch == '}' {
                depth -= 1;
                if found_open && depth == 0 {
                    return i + 1; // 1-based
                }
            }
        }
    }
    // fallback: start + 20
    (start_line + 20).min(total)
}

/// Collect all contracts in the inheritance chain (self + base_contracts, recursively).
fn collect_inheritance_chain<'a>(
    evm_metadata: &'a EvmBatMetadata,
    contract_name: &str,
) -> Vec<&'a crate::batbelt::evm::metadata::bat_metadata::ContractMetadata> {
    let mut chain = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    queue.push_back(contract_name.to_string());

    while let Some(name) = queue.pop_front() {
        if visited.contains(&name) {
            continue;
        }
        visited.insert(name.clone());
        if let Some(c) = evm_metadata.get_contract_by_name(&name) {
            chain.push(c);
            for base in &c.base_contracts {
                queue.push_back(base.clone());
            }
        }
    }
    chain
}

/// Extract only the lines inside the function body (after the opening `{`),
/// excluding the function signature, modifiers, and closing `}`.
fn extract_body_only_lines(func_lines: &[String]) -> Vec<String> {
    let mut body_lines = Vec::new();
    let mut found_open = false;
    let mut depth: i32 = 0;

    for line in func_lines {
        for ch in line.chars() {
            if ch == '{' {
                depth += 1;
                if !found_open {
                    found_open = true;
                    continue; // skip the opening brace line for body scanning
                }
            } else if ch == '}' {
                depth -= 1;
            }
        }
        if found_open && depth > 0 {
            body_lines.push(line.clone());
        }
    }
    body_lines
}

/// Resolve direct dependencies for a function by scanning its body for internal calls
/// and modifiers. Searches the full inheritance chain for modifier definitions and
/// internal functions. Returns (metadata_id, name, file_path, line, end_line).
fn resolve_evm_function_deps(
    evm_metadata: &EvmBatMetadata,
    contract_name: &str,
    func_metadata_id: &str,
    func_modifiers: &[String],
    func_lines: &[String],
    _contract_file_path: &str,
) -> Vec<(String, String, String, usize, usize)> {
    let mut deps: Vec<(String, String, String, usize, usize)> = Vec::new();
    let mut seen_names: HashSet<String> = HashSet::new();
    let mut seen_ids: HashSet<String> = HashSet::new();

    // Collect all contracts in the inheritance chain
    let chain = collect_inheritance_chain(evm_metadata, contract_name);

    // 1. Resolve modifiers as dependencies (search whole inheritance chain)
    for mod_name in func_modifiers {
        for c in &chain {
            if let Some(mod_def) = c.modifiers.iter().find(|m| m.name == *mod_name) {
                let mod_id = format!("modifier_{}_{}", c.name, mod_name);
                if !seen_ids.contains(&mod_id) {
                    seen_ids.insert(mod_id.clone());
                    seen_names.insert(mod_name.clone());
                    let end = find_function_end_line(&c.file_path, mod_def.line);
                    deps.push((
                        mod_id,
                        format!("modifier {}", mod_name),
                        c.file_path.clone(),
                        mod_def.line,
                        end,
                    ));
                }
                break; // found it, stop searching chain
            }
        }
    }

    // 2. Resolve internal function calls from body ONLY (after opening `{`)
    // This avoids false positives from the function signature, parameter names,
    // or modifier calls in the signature.
    let body_only = extract_body_only_lines(func_lines);

    for c in &chain {
        for func_meta in &c.functions {
            if func_meta.metadata_id == func_metadata_id {
                continue; // skip self
            }
            // Skip functions that share name with an already-resolved modifier
            if seen_names.contains(&func_meta.name) {
                continue;
            }
            let call_pattern = format!("{}(", func_meta.name);
            let is_called = body_only
                .iter()
                .any(|line| line.contains(&call_pattern));
            if is_called && !seen_ids.contains(&func_meta.metadata_id) {
                seen_ids.insert(func_meta.metadata_id.clone());
                seen_names.insert(func_meta.name.clone());
                let end = if func_meta.end_line > 0 {
                    func_meta.end_line
                } else {
                    find_function_end_line(&c.file_path, func_meta.line)
                };
                deps.push((
                    func_meta.metadata_id.clone(),
                    func_meta.name.clone(),
                    c.file_path.clone(),
                    func_meta.line,
                    end,
                ));
            }
        }
    }

    deps
}

/// Find the start of NatSpec/documentation comments above a function definition.
/// Walks backwards from `func_start_line` (1-based) to include `/** ... */` or `///` blocks.
fn find_doc_start_line(file_path: &str, func_start_line: usize) -> usize {
    let content = std::fs::read_to_string(file_path).unwrap_or_default();
    let lines: Vec<&str> = content.lines().collect();
    if func_start_line <= 1 {
        return func_start_line;
    }

    let mut doc_start = func_start_line; // 1-based
    let mut i = func_start_line - 2; // 0-based index of line above function

    // Skip blank lines immediately above the function
    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.is_empty() {
            if i == 0 {
                break;
            }
            i -= 1;
        } else {
            break;
        }
    }

    if i >= lines.len() {
        return doc_start;
    }

    let trimmed = lines[i].trim();

    // Check for `*/` ending a block comment
    if trimmed.ends_with("*/") {
        // Walk backwards to find the `/**` or `/*`
        loop {
            doc_start = i + 1; // 1-based
            if lines[i].trim().starts_with("/**") || lines[i].trim().starts_with("/*") {
                break;
            }
            if i == 0 {
                break;
            }
            i -= 1;
        }
    } else if trimmed.starts_with("///") {
        // Walk backwards through consecutive `///` lines
        doc_start = i + 1;
        while i > 0 {
            i -= 1;
            if lines[i].trim().starts_with("///") {
                doc_start = i + 1;
            } else {
                break;
            }
        }
    }

    doc_start
}

/// Deploy code-overhaul screenshots for a single EVM entry point into its Miro frame.
///
/// Deploys: entry point screenshot, validations screenshot (with header),
/// and BFS dependency screenshots (modifiers + internal calls) with connectors.
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
    // Include NatSpec documentation above the function
    let ep_start_line = find_doc_start_line(&contract.file_path, func.line);
    let ep_end_line = if func.end_line > 0 {
        func.end_line
    } else {
        find_function_end_line(&contract.file_path, func.line)
    };

    // Use .js extension so silicon renders with JavaScript syntax highlighting
    // (Dracula + JS gives the best color contrast for Solidity code)
    let ep_sc = SourceCodeParser::new(
        format!("ep_{}.js", func.name),
        contract.file_path.clone(),
        ep_start_line,
        ep_end_line,
    );

    let ep_image = ep_sc
        .deploy_screenshot_to_miro_frame(
            co_miro_frame.clone(),
            1200,
            600,
            SourceCodeScreenshotOptions {
                include_path: true,
                offset_to_start_line: true,
                filter_comments: false,
                font_size: Some(28),
                filters: None,
                show_line_number: true,
            },
        )
        .await
        .change_context(EvmMiroError)?;

    let entry_point_image_id = ep_image.item_id.clone();

    // 2. BFS deployment of dependency screenshots
    // Resolve deps from function body + modifiers (not from function_dependencies which may be empty)
    // Track both IDs and names to prevent deploying virtual parent versions of overridden functions
    let mut deployed_function_ids: HashSet<String> = HashSet::new();
    deployed_function_ids.insert(ep.function_metadata_id.clone());
    let mut deployed_function_names: HashSet<String> = HashSet::new();
    deployed_function_names.insert(func.name.clone());

    let mut id_to_image: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    id_to_image.insert(ep.function_metadata_id.clone(), entry_point_image_id.clone());

    let mut bfs_queue: VecDeque<(String, String, Vec<String>, Vec<String>, String)> =
        VecDeque::new();

    // Read body lines for the entry point function
    let ep_body_lines: Vec<String> = {
        let file_content =
            std::fs::read_to_string(&contract.file_path).unwrap_or_default();
        let lines: Vec<&str> = file_content.lines().collect();
        let start = if func.line > 0 { func.line - 1 } else { 0 };
        let end = ep_end_line.min(lines.len());
        lines[start..end].iter().map(|s| s.to_string()).collect()
    };

    bfs_queue.push_back((
        ep.function_metadata_id.clone(),
        func.name.clone(),
        func.modifiers.clone(),
        ep_body_lines,
        contract.file_path.clone(),
    ));

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

    while let Some((caller_id, caller_name, caller_modifiers, caller_body, caller_file)) =
        bfs_queue.pop_front()
    {
        // Resolve deps dynamically from body + modifiers
        let new_dep_functions = resolve_evm_function_deps(
            &evm_metadata,
            &ep.contract_name,
            &caller_id,
            &caller_modifiers,
            &caller_body,
            &caller_file,
        );

        // Filter already deployed (by ID or by name to prevent virtual/override duplicates)
        let new_deps: Vec<_> = new_dep_functions
            .into_iter()
            .filter(|(id, name, _, _, _)| {
                !deployed_function_ids.contains(id) && !deployed_function_names.contains(name)
            })
            .collect();

        if new_deps.is_empty() {
            continue;
        }

        let (arrow_hex, arrow_name) = DEP_ARROW_COLORS[color_index % DEP_ARROW_COLORS.len()];
        color_index += 1;

        let prompt = format!(
            "Press Enter to deploy {} dependencies of `{}` (arrow color: {})",
            new_deps.len(),
            caller_name,
            arrow_name
        );
        BatDialoguer::input_with_default(prompt, "".to_string())
            .change_context(EvmMiroError)?;

        for (idx, (dep_id, dep_name, dep_path, dep_line, dep_end)) in
            new_deps.iter().enumerate()
        {
            // .js extension → JavaScript syntax highlighting in silicon (best for Solidity)
            let dep_sc = SourceCodeParser::new(
                format!("dep_{}.js", dep_name),
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
            // Store the base name (strip "modifier " prefix) so virtual/override
            // and modifier-as-function duplicates are caught
            let base_name = dep_name.strip_prefix("modifier ").unwrap_or(dep_name);
            deployed_function_names.insert(base_name.to_string());
            dependency_image_ids.push(dep_image.item_id.clone());

            // Read body of dep for further BFS (only for non-modifier deps)
            let dep_body: Vec<String> = {
                let fc = std::fs::read_to_string(dep_path).unwrap_or_default();
                let lines: Vec<&str> = fc.lines().collect();
                let s = if *dep_line > 0 { dep_line - 1 } else { 0 };
                let e = (*dep_end).min(lines.len());
                lines[s..e].iter().map(|l| l.to_string()).collect()
            };
            // Dep functions don't have modifiers in this context (we'd need to look them up)
            bfs_queue.push_back((
                dep_id.clone(),
                dep_name.clone(),
                vec![],
                dep_body,
                dep_path.clone(),
            ));
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
            frame.validations_image_id = String::new();
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
