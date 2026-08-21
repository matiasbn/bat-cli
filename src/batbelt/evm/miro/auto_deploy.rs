//! Fully automatic deployment of an EVM entry point's dependency graph to Miro.
//!
//! One frame per entry point (public/external function). Inside it, every
//! function in the call graph is rendered, measured, laid out and uploaded
//! already positioned, and every call site gets a connector anchored to the
//! exact line of the caller that makes the call.
//!
//! Nothing is dragged by hand: the pipeline is
//!
//! ```text
//! metadata → graph → PNGs → measure → scale → layout → frame → images → connectors → persist
//! ```

use std::collections::{HashMap, HashSet, VecDeque};

use colored::Colorize;
use error_stack::{IntoReport, Report, ResultExt};

use crate::batbelt::evm::metadata::bat_metadata::{
    AutoDeployedFrame, ContractMetadata, EvmBatMetadata, FunctionMetadata, ShelfState,
};
use crate::batbelt::evm::miro::EvmMiroError;
use crate::batbelt::evm::parser::call_resolver::extract_call_sites_from_source;
use crate::batbelt::evm::types::EvmContractType;
use crate::batbelt::miro::client::{ConnectorStyle, MiroClient, RelativeAnchor};
use crate::batbelt::miro::layout::{
    layout_graph, GraphLayout, LayoutConfig, LayoutEdge, LayoutNode, ShelfAllocator,
};
use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::path::BatFolder;
use crate::batbelt::silicon;

type Result<T> = error_stack::Result<T, EvmMiroError>;

/// Gap left between the end of the code and the connector anchor, in characters.
const ANCHOR_GAP_CHARS: f64 = 1.0;
/// Vertical distance from the top of the callee where its signature sits: line
/// index 2 of the image, because `include_path` prepends `// path` plus a blank.
const SIGNATURE_LINE_INDEX: usize = 2;
/// Number of lines `include_path` prepends to the rendered content.
const PATH_HEADER_LINES: usize = 2;
/// Vertical margin left between the existing board content and the region we
/// reserve for automatic deployments.
const REGION_MARGIN: f64 = 5_000.0;

/// Side of the invisible square the connector attaches to, in board units.
/// Small enough that the arrow head reads as landing on the token itself.
const ANCHOR_MARKER_SIZE: f64 = 24.0;

/// Connector colors, cycled per depth so sibling levels stay distinguishable.
const DEPTH_COLORS: &[&str] = &[
    "#2d9bf0", "#f24726", "#8fd14f", "#fac710", "#a259ff", "#12cdd4", "#ff8c00", "#e6007a",
];

#[derive(Debug, Clone)]
pub struct AutoDeployOptions {
    /// Deploy only this entry point (`name` or `Contract.name`).
    pub entry_point: Option<String>,
    /// Deploy every entry point in the project.
    pub all: bool,
    /// Stop expanding the call graph past this depth.
    pub max_depth: usize,
    /// Hard cap on the number of screenshots per frame.
    pub max_nodes: usize,
    /// Compute and print the layout without touching Miro.
    pub dry_run: bool,
    /// Include contracts coming from `lib/`.
    pub include_external: bool,
    /// Delete the previous deployment of this entry point before redeploying.
    pub replace: bool,
    /// Compose a local preview PNG of the frame at this path.
    pub preview: Option<String>,
    /// Connector thickness in dp, 1 to 24. Miro's UI snaps this to its own
    /// preset levels, so 12 lands on roughly "level 5".
    pub stroke_width: u32,
}

impl Default for AutoDeployOptions {
    fn default() -> Self {
        Self {
            entry_point: None,
            all: false,
            max_depth: 4,
            max_nodes: 60,
            dry_run: false,
            include_external: false,
            replace: false,
            preview: None,
            stroke_width: 8,
        }
    }
}

/// One rendered function in the graph.
#[derive(Debug, Clone)]
struct GraphNode {
    id: String,
    label: String,
    file_path: String,
    /// 1-based, inclusive, in the source file.
    start_line: usize,
    end_line: usize,
    depth: usize,
    font_size: usize,
    /// Filled in during the render phase.
    png_path: String,
    png_width: u32,
    png_height: u32,
    /// Text of every line rendered in the image, including the path header.
    rendered_lines: Vec<String>,
    /// `line_offset` handed to silicon, needed to reproduce the line-number gutter.
    line_offset: usize,
}

impl GraphNode {
    fn board_width(&self) -> f64 {
        self.png_width as f64 * BOARD_UNITS_PER_PIXEL
    }

    fn board_height(&self) -> f64 {
        self.png_height as f64 * BOARD_UNITS_PER_PIXEL
    }
}

/// One call site: caller, callee, and the line of the caller it happens on.
#[derive(Debug, Clone)]
struct GraphEdge {
    from: String,
    to: String,
    /// 1-based line inside the caller's captured slice.
    line_in_slice: usize,
    /// 0-based column where the called name starts on that line.
    column: usize,
    /// The token the connector should point at, e.g. `wadMul` in
    /// `MathLib.wadMul(...)`.
    symbol: String,
}

/// How many board units one rendered pixel becomes.
///
/// Constant on purpose. Forcing every screenshot to a fixed board width instead
/// would blow up a narrow capture and shrink a wide one — a 530 px image
/// stretched to 1200 and a 1468 px image squeezed to 1200 end up with nearly 3x
/// difference in text size inside the same layer. Keeping the ratio fixed means
/// the only thing that changes the text size is the font used to render it.
const BOARD_UNITS_PER_PIXEL: f64 = 1.0;

/// Font per depth: the entry point is rendered biggest and leaves smallest, so a
/// deep graph stays readable. Width now follows from the code itself.
fn font_for_depth(depth: usize) -> usize {
    match depth {
        0 => 32,
        1 => 26,
        _ => 22,
    }
}

pub async fn run(options: AutoDeployOptions) -> Result<()> {
    let metadata = EvmBatMetadata::read_metadata().change_context(EvmMiroError)?;

    let targets = select_entry_points(&metadata, &options)?;
    if targets.is_empty() {
        return Err(Report::new(EvmMiroError)
            .attach_printable("no entry point matched; run `bat-cli sonar` first"));
    }

    if options.all && !options.dry_run && targets.len() > 1 {
        println!(
            "{} deploying {} entry points at once puts thousands of objects on the\nboard, which Miro starts to slow down past a thousand. Reviewing happens one\nentry point at a time, so consider deploying on demand instead.",
            "warning:".yellow(),
            targets.len()
        );
        if !BatDialoguer::select_yes_or_no("Deploy all of them anyway?".to_string())
            .change_context(EvmMiroError)?
        {
            return Ok(());
        }
    }

    println!(
        "Auto-deploying {} entry point(s){}",
        targets.len().to_string().green(),
        if options.dry_run {
            " (dry run, nothing is sent to Miro)".yellow().to_string()
        } else {
            String::new()
        }
    );

    // One client for the whole batch, so the credit budget is shared.
    let client = if options.dry_run {
        None
    } else {
        Some(
            MiroClient::new_refreshed()
                .await
                .change_context(EvmMiroError)?,
        )
    };

    // The board is scanned at most once, to pick the region origin.
    let mut allocator = if options.dry_run {
        ShelfAllocator::new(0.0, 0.0)
    } else {
        resolve_allocator(client.as_ref().unwrap(), &metadata).await?
    };

    for (contract_name, function_name) in targets {
        deploy_one(
            &metadata,
            &contract_name,
            &function_name,
            &options,
            client.as_ref(),
            &mut allocator,
        )
        .await?;

        // Persist the cursor after every entry point, not once at the end: a run
        // over a whole project is long enough to be interrupted, and a lost
        // cursor would place the next batch on top of the frames already there.
        if !options.dry_run {
            let state = ShelfState::from(&allocator);
            EvmBatMetadata::update_metadata(|m| m.miro.auto.region = Some(state.clone()))
                .change_context(EvmMiroError)?;
        }
    }

    Ok(())
}

/// Which entry points to deploy.
///
/// Deploying a whole project at once is deliberately not the default: an audit
/// is reviewed one entry point at a time, and a project of any size would put
/// thousands of objects on a board that Miro starts to slow down past a
/// thousand. With no `--entry-point` and no `--all`, ask.
fn select_entry_points(
    metadata: &EvmBatMetadata,
    options: &AutoDeployOptions,
) -> Result<Vec<(String, String)>> {
    // `EntryPointMetadata::name` is stored as `Contract.function`, so strip the
    // prefix to get the bare function name used to look it up in the contract.
    let mut all: Vec<(String, String)> = metadata
        .entry_points
        .iter()
        .map(|ep| {
            let function = ep
                .name
                .strip_prefix(&format!("{}.", ep.contract_name))
                .unwrap_or(&ep.name)
                .to_string();
            (ep.contract_name.clone(), function)
        })
        .collect();
    all.sort();
    all.dedup();

    if options.all {
        return Ok(all);
    }

    if let Some(wanted) = &options.entry_point {
        return Ok(all
            .into_iter()
            .filter(|(contract, function)| {
                *function == *wanted || format!("{contract}.{function}") == *wanted
            })
            .collect());
    }

    if all.is_empty() {
        return Ok(all);
    }

    // Mark the ones already on the board, so a long list still shows what is
    // left to do.
    let deployed: HashSet<String> = metadata
        .miro
        .auto
        .frames
        .iter()
        .map(|frame| frame.entry_point.clone())
        .collect();
    let labels: Vec<String> = all
        .iter()
        .map(|(contract, function)| {
            let title = format!("{contract}.{function}");
            if deployed.contains(&title) {
                format!("{title} {}", "(deployed)".green())
            } else {
                title
            }
        })
        .collect();

    let selection = BatDialoguer::fuzzy_select(
        "Select the entry point to deploy:".to_string(),
        labels,
    )
    .change_context(EvmMiroError)?;

    Ok(vec![all[selection].clone()])
}

/// Reserve (or recover) the board region the automatic frames live in.
async fn resolve_allocator(
    client: &MiroClient,
    metadata: &EvmBatMetadata,
) -> Result<ShelfAllocator> {
    if let Some(state) = &metadata.miro.auto.region {
        return Ok(state.to_allocator());
    }

    println!("Scanning the board once to reserve a region for automatic frames...");
    let frames = client.list_frames().await.change_context(EvmMiroError)?;

    let (origin_x, origin_y) = if frames.is_empty() {
        (0.0, 0.0)
    } else {
        let bottom = frames.iter().map(|f| f.bottom()).fold(f64::MIN, f64::max);
        let left = frames.iter().map(|f| f.left()).fold(f64::MAX, f64::min);
        (left, bottom + REGION_MARGIN)
    };

    println!(
        "  region origin: ({}, {}) — below {} existing frame(s)",
        origin_x.round(),
        origin_y.round(),
        frames.len()
    );
    Ok(ShelfAllocator::new(origin_x, origin_y))
}

async fn deploy_one(
    metadata: &EvmBatMetadata,
    contract_name: &str,
    function_name: &str,
    options: &AutoDeployOptions,
    client: Option<&MiroClient>,
    allocator: &mut ShelfAllocator,
) -> Result<()> {
    let title = format!("{contract_name}.{function_name}");
    println!("\n{} {}", "▸".blue(), title.bold());

    let (mut nodes, edges, truncated) = build_graph(metadata, contract_name, function_name, options)?;
    if nodes.is_empty() {
        println!("  no function metadata found, skipping");
        return Ok(());
    }
    if truncated > 0 {
        println!(
            "  {} {} node(s) not expanded (max-depth {} / max-nodes {})",
            "note:".yellow(),
            truncated,
            options.max_depth,
            options.max_nodes
        );
    }

    render_and_measure(&mut nodes)?;

    let layout_nodes: Vec<LayoutNode> = nodes
        .iter()
        .map(|node| LayoutNode {
            id: node.id.clone(),
            width: node.board_width(),
            height: node.board_height(),
        })
        .collect();

    let by_id: HashMap<&str, &GraphNode> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let anchors = compute_anchors(&nodes, &edges);
    let layout_edges: Vec<LayoutEdge> = edges
        .iter()
        .zip(anchors.iter())
        .map(|(edge, anchor)| LayoutEdge {
            from: edge.from.clone(),
            to: edge.to.clone(),
            from_line_fraction: anchor.y_fraction,
        })
        .collect();

    let root_id = nodes[0].id.clone();
    let layout = layout_graph(&root_id, &layout_nodes, &layout_edges, LayoutConfig::default());

    // Reserve the slot in both modes, so a dry run shows the real sequence of
    // board positions instead of repeating the first one.
    let (frame_x, frame_y) = allocator.place(layout.frame_width, layout.frame_height);

    if let Some(preview_path) = &options.preview {
        let path = if options.all {
            let safe = title.replace(['.', '/'], "_");
            format!("{}/{}.png", preview_path.trim_end_matches('/'), safe)
        } else {
            preview_path.clone()
        };
        render_preview(&nodes, &edges, &anchors, &layout, &path)?;
        println!("  preview written to {}", path.blue());
    }

    if options.dry_run {
        print_dry_run(&nodes, &edges, &anchors, &layout, (frame_x, frame_y));
        cleanup(&nodes);
        return Ok(());
    }

    let client = client.expect("client is present when not in dry-run mode");

    if options.replace {
        remove_previous_deployment(client, &title).await?;
    }
    let frame_id = client
        .create_frame(
            &format!("auto: {title}"),
            frame_x,
            frame_y,
            layout.frame_width,
            layout.frame_height,
        )
        .await
        .change_context(EvmMiroError)?;
    println!(
        "  frame {} ({}x{}) at ({}, {})",
        frame_id.green(),
        layout.frame_width.round(),
        layout.frame_height.round(),
        frame_x.round(),
        frame_y.round()
    );

    // Images, already positioned and parented — one call each, no follow-up PATCH.
    let mut image_ids: HashMap<String, String> = HashMap::new();
    for node in &nodes {
        let placed = match layout.node(&node.id) {
            Some(placed) => placed,
            None => continue,
        };
        let image_id = client
            .create_image_in_frame(
                &node.png_path,
                &frame_id,
                &node.label,
                placed.x,
                placed.y,
                placed.width,
            )
            .await
            .change_context(EvmMiroError)?;
        println!("    {} {}", "✓".green(), node.label);
        image_ids.insert(node.id.clone(), image_id);
    }

    // Connectors, one per call site. Each starts on an invisible marker sitting
    // on the called token, because Miro clips a connector at the boundary of the
    // item it attaches to: anchoring inside the screenshot itself would push the
    // arrow head out to the screenshot's border.
    let back_edges: HashSet<(String, String)> = layout.back_edges.iter().cloned().collect();
    let mut connector_ids = Vec::new();
    let mut marker_ids = Vec::new();
    for (edge, start_anchor) in edges.iter().zip(anchors.iter()) {
        let (Some(start_id), Some(end_id)) =
            (image_ids.get(&edge.from), image_ids.get(&edge.to))
        else {
            continue;
        };
        let caller = match by_id.get(edge.from.as_str()) {
            Some(node) => *node,
            None => continue,
        };
        let callee = match by_id.get(edge.to.as_str()) {
            Some(node) => *node,
            None => continue,
        };

        let end_anchor = RelativeAnchor::new(
            0.0,
            silicon::line_geometry(Some(callee.font_size))
                .line_center_fraction(SIGNATURE_LINE_INDEX, callee.png_height),
        );

        let Some(caller_placed) = layout.node(&edge.from) else {
            continue;
        };
        let marker_id = client
            .create_anchor_marker(
                &frame_id,
                caller_placed.x - caller_placed.width / 2.0
                    + caller_placed.width * start_anchor.x_fraction,
                caller_placed.y - caller_placed.height / 2.0
                    + caller_placed.height * start_anchor.y_fraction,
                ANCHOR_MARKER_SIZE,
            )
            .await
            .change_context(EvmMiroError)?;
        marker_ids.push(marker_id.clone());

        let source_line = caller.start_line + edge.line_in_slice - 1;
        let style = ConnectorStyle {
            stroke_color: DEPTH_COLORS[caller.depth % DEPTH_COLORS.len()].to_string(),
            stroke_width: options.stroke_width.to_string(),
            dashed: back_edges.contains(&(edge.from.clone(), edge.to.clone())),
            caption: Some(format!("<p>L{source_line}</p>")),
            arrow_at_start: true,
        };

        let _ = start_id;
        let connector_id = client
            .create_connector(
                &marker_id,
                // Centre, so Miro routes into the marker from above and the
                // connector keeps its elbowed look. The head then points down
                // at the token: its tip is exact, but its body is roughly as
                // long as a line is tall at this stroke width, so it reads as
                // sitting slightly high. Anchoring on the right edge would lay
                // the head along the line instead, at the cost of a straight
                // horizontal run into the token, which reads worse.
                // `--stroke-width` shrinks the head if the offset matters more.
                RelativeAnchor::new(0.5, 0.5),
                end_id,
                end_anchor,
                style,
            )
            .await
            .change_context(EvmMiroError)?;
        connector_ids.push(connector_id);
    }
    println!("    {} {} connector(s)", "✓".green(), connector_ids.len());

    let frame_url = client.frame_url(&frame_id);
    let record = AutoDeployedFrame {
        entry_point: title.clone(),
        frame_id: frame_id.clone(),
        frame_url: frame_url.clone(),
        x: frame_x,
        y: frame_y,
        width: layout.frame_width,
        height: layout.frame_height,
        images: image_ids.into_iter().collect(),
        connector_ids,
        marker_ids,
    };
    EvmBatMetadata::update_metadata(|m| {
        m.miro.auto.frames.retain(|f| f.entry_point != record.entry_point);
        m.miro.auto.frames.push(record.clone());
    })
    .change_context(EvmMiroError)?;

    println!("  {}", frame_url.blue());
    cleanup(&nodes);
    Ok(())
}

/// Delete the frame, images and connectors left by an earlier run, so iterating
/// on the layout does not pile up duplicates on the board.
///
/// Connectors go first: deleting an item its connector still points at leaves
/// the connector dangling.
async fn remove_previous_deployment(client: &MiroClient, entry_point: &str) -> Result<()> {
    let metadata = EvmBatMetadata::read_metadata().change_context(EvmMiroError)?;
    let Some(previous) = metadata
        .miro
        .auto
        .frames
        .iter()
        .find(|frame| frame.entry_point == entry_point)
        .cloned()
    else {
        return Ok(());
    };

    println!(
        "  replacing the previous deployment ({} image(s), {} connector(s), {} marker(s))",
        previous.images.len(),
        previous.connector_ids.len(),
        previous.marker_ids.len()
    );
    for connector_id in &previous.connector_ids {
        client
            .delete_connector(connector_id)
            .await
            .change_context(EvmMiroError)?;
    }
    for marker_id in &previous.marker_ids {
        client
            .delete_item(marker_id)
            .await
            .change_context(EvmMiroError)?;
    }
    for (_, image_id) in &previous.images {
        client
            .delete_item(image_id)
            .await
            .change_context(EvmMiroError)?;
    }
    client
        .delete_item(&previous.frame_id)
        .await
        .change_context(EvmMiroError)?;

    EvmBatMetadata::update_metadata(|m| {
        m.miro.auto.frames.retain(|f| f.entry_point != entry_point);
    })
    .change_context(EvmMiroError)?;
    Ok(())
}

/// Anchors for every edge, in the same order as `edges`.
///
/// Call sites that share a line would otherwise start from the exact same
/// point, so they are fanned out slightly inside the line's height.
fn compute_anchors(nodes: &[GraphNode], edges: &[GraphEdge]) -> Vec<RelativeAnchor> {
    let by_id: HashMap<&str, &GraphNode> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    // Keyed by line *and* column: two calls on one line now land on their own
    // tokens, so only genuinely identical positions need fanning out.
    let mut occurrences: HashMap<(&str, usize, usize), usize> = HashMap::new();
    for edge in edges {
        *occurrences
            .entry((edge.from.as_str(), edge.line_in_slice, edge.column))
            .or_insert(0) += 1;
    }

    let mut seen: HashMap<(&str, usize, usize), usize> = HashMap::new();
    edges
        .iter()
        .map(|edge| {
            let Some(node) = by_id.get(edge.from.as_str()) else {
                return RelativeAnchor::new(1.0, 0.5);
            };
            let key = (edge.from.as_str(), edge.line_in_slice, edge.column);
            let index = seen.entry(key).or_insert(0);
            let position = *index;
            *index += 1;
            let total = occurrences.get(&key).copied().unwrap_or(1);

            let mut anchor = caller_anchor(node, edge);
            if total > 1 && node.png_height > 0 {
                // Spread the siblings across the line's own height, so each
                // connector still visibly belongs to that line.
                let line_height = silicon::line_geometry(Some(node.font_size)).line_height as f64;
                let spread = line_height * 0.6 / node.png_height as f64;
                let offset = (position as f64 - (total as f64 - 1.0) / 2.0) * spread
                    / (total as f64 - 1.0).max(1.0);
                anchor = RelativeAnchor::new(anchor.x_fraction, anchor.y_fraction + offset);
            }
            anchor
        })
        .collect()
}

/// Anchor for the connector's caller end: on the token that makes the call.
///
/// The AST gives the column of the callee's own name, so the anchor lands on
/// `wadMul` in `MathLib.wadMul(amount, price(asset))` and on `price` a few
/// columns later — two calls on one line get two distinct anchors with no
/// guessing involved.
fn caller_anchor(node: &GraphNode, edge: &GraphEdge) -> RelativeAnchor {
    let line_index = edge.line_in_slice - 1 + PATH_HEADER_LINES;
    let geometry = silicon::line_geometry(Some(node.font_size));
    let y_fraction = geometry.line_center_fraction(line_index, node.png_height);

    let line_text = node
        .rendered_lines
        .get(line_index)
        .cloned()
        .unwrap_or_default();

    // Prefer the recorded column; fall back to searching the line, and finally
    // to the end of the line if the token is nowhere to be found.
    let start = if line_text
        .get(edge.column..edge.column + edge.symbol.len())
        .map(|found| found == edge.symbol)
        .unwrap_or(false)
    {
        Some(edge.column)
    } else {
        line_text.find(&edge.symbol)
    };

    let x_fraction = match (start, node.png_width) {
        (Some(column), width) if width > 0 => {
            let text_width = |text: &str| {
                silicon::line_end_x(
                    Some(node.font_size),
                    true,
                    node.rendered_lines.len(),
                    node.line_offset,
                    text,
                ) as f64
            };
            // Aim at the middle of the token so the head visibly sits on it.
            let before = text_width(&line_text[..column]);
            let through = text_width(&line_text[..column + edge.symbol.len()]);
            (before + through) / 2.0 / width as f64
        }
        (_, width) if width > 0 => {
            silicon::line_end_x(
                Some(node.font_size),
                true,
                node.rendered_lines.len(),
                node.line_offset,
                &line_text,
            ) as f64
                / width as f64
        }
        _ => 1.0,
    };

    RelativeAnchor::new(x_fraction, y_fraction)
}

/// BFS over the call graph, keeping every call site with its line.
fn build_graph(
    metadata: &EvmBatMetadata,
    contract_name: &str,
    function_name: &str,
    options: &AutoDeployOptions,
) -> Result<(Vec<GraphNode>, Vec<GraphEdge>, usize)> {
    let Some(root_function) = find_function(metadata, contract_name, function_name) else {
        return Ok((Vec::new(), Vec::new(), 0));
    };
    let Some(root_contract) = metadata.get_contract_by_name(contract_name) else {
        return Ok((Vec::new(), Vec::new(), 0));
    };

    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut truncated = 0usize;

    let root_id = node_id(contract_name, function_name);
    seen.insert(root_id.clone());
    nodes.push(make_node(
        root_id.clone(),
        format!("{contract_name}.{function_name}"),
        root_contract,
        &root_function,
        0,
    ));

    let mut queue: VecDeque<(String, String, String, usize)> = VecDeque::new();
    queue.push_back((
        root_id,
        contract_name.to_string(),
        function_name.to_string(),
        0,
    ));

    while let Some((caller_id, caller_contract, caller_function, depth)) = queue.pop_front() {
        if depth >= options.max_depth {
            continue;
        }
        let Some(contract) = metadata.get_contract_by_name(&caller_contract) else {
            continue;
        };
        let Some(function) = find_function(metadata, &caller_contract, &caller_function) else {
            continue;
        };
        let slice = read_slice(&contract.file_path, function.line, function_end(&function, contract));

        // Modifiers count as dependencies; their call site is the signature line
        // where the modifier name appears.
        for modifier_name in &function.modifiers {
            if let Some((owner, definition)) = find_modifier(metadata, &caller_contract, modifier_name)
            {
                let target_id = format!("modifier:{}::{}", owner.name, definition.name);
                let line_in_slice = slice
                    .iter()
                    .position(|line| line.contains(modifier_name))
                    .map(|index| index + 1)
                    .unwrap_or(1);
                edges.push(GraphEdge {
                    from: caller_id.clone(),
                    to: target_id.clone(),
                    line_in_slice,
                    column: slice
                        .get(line_in_slice - 1)
                        .and_then(|line| line.find(modifier_name.as_str()))
                        .unwrap_or(0),
                    symbol: modifier_name.clone(),
                });
                if seen.insert(target_id.clone()) {
                    if nodes.len() >= options.max_nodes {
                        truncated += 1;
                        continue;
                    }
                    nodes.push(make_modifier_node(
                        target_id,
                        owner,
                        definition.name.clone(),
                        definition.line,
                        if definition.end_line > 0 {
                            definition.end_line
                        } else {
                            definition.line + 6
                        },
                        depth + 1,
                    ));
                }
            }
        }

        for call in extract_call_sites_from_source(&slice.join("\n")) {
            let Some((target_contract, target_function)) =
                resolve_call(metadata, contract, &call.name, options)
            else {
                continue;
            };
            if target_contract.name == caller_contract && target_function.name == caller_function {
                continue; // direct recursion on itself, nothing to draw
            }
            let target_id = node_id(&target_contract.name, &target_function.name);
            edges.push(GraphEdge {
                from: caller_id.clone(),
                to: target_id.clone(),
                line_in_slice: call.line,
                column: call.column,
                symbol: call.symbol.clone(),
            });

            if seen.insert(target_id.clone()) {
                if nodes.len() >= options.max_nodes {
                    truncated += 1;
                    continue;
                }
                nodes.push(make_node(
                    target_id.clone(),
                    format!("{}.{}", target_contract.name, target_function.name),
                    target_contract,
                    &target_function,
                    depth + 1,
                ));
                queue.push_back((
                    target_id,
                    target_contract.name.clone(),
                    target_function.name.clone(),
                    depth + 1,
                ));
            }
        }
    }

    // Drop edges pointing at nodes that were cut by the caps.
    let known: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    edges.retain(|edge| known.contains(edge.from.as_str()) && known.contains(edge.to.as_str()));

    Ok((nodes, edges, truncated))
}

fn node_id(contract: &str, function: &str) -> String {
    format!("{contract}::{function}")
}

fn make_node(
    id: String,
    label: String,
    contract: &ContractMetadata,
    function: &FunctionMetadata,
    depth: usize,
) -> GraphNode {
    GraphNode {
        id,
        label,
        file_path: contract.file_path.clone(),
        start_line: function.line,
        end_line: function_end(function, contract),
        depth,
        font_size: font_for_depth(depth),
        png_path: String::new(),
        png_width: 0,
        png_height: 0,
        rendered_lines: Vec::new(),
        line_offset: 0,
    }
}

fn make_modifier_node(
    id: String,
    contract: &ContractMetadata,
    name: String,
    start_line: usize,
    end_line: usize,
    depth: usize,
) -> GraphNode {
    GraphNode {
        id,
        label: format!("{}.{} (modifier)", contract.name, name),
        file_path: contract.file_path.clone(),
        start_line,
        end_line,
        depth,
        font_size: font_for_depth(depth),
        png_path: String::new(),
        png_width: 0,
        png_height: 0,
        rendered_lines: Vec::new(),
        line_offset: 0,
    }
}

fn function_end(function: &FunctionMetadata, contract: &ContractMetadata) -> usize {
    if function.end_line > 0 {
        return function.end_line;
    }
    // Fall back to a brace scan when sonar did not record the end line.
    let content = std::fs::read_to_string(&contract.file_path).unwrap_or_default();
    let lines: Vec<&str> = content.lines().collect();
    let mut depth = 0i32;
    let mut started = false;
    for (index, line) in lines.iter().enumerate().skip(function.line.saturating_sub(1)) {
        for character in line.chars() {
            match character {
                '{' => {
                    depth += 1;
                    started = true;
                }
                '}' => depth -= 1,
                _ => {}
            }
        }
        if started && depth <= 0 {
            return index + 1;
        }
    }
    function.line
}

fn find_function(
    metadata: &EvmBatMetadata,
    contract_name: &str,
    function_name: &str,
) -> Option<FunctionMetadata> {
    let contract = metadata.get_contract_by_name(contract_name)?;
    if let Some(function) = contract.functions.iter().find(|f| f.name == function_name) {
        return Some(function.clone());
    }
    // Inherited function.
    for base in &contract.base_contracts {
        if let Some(found) = find_function(metadata, base, function_name) {
            return Some(found);
        }
    }
    None
}

fn find_modifier<'a>(
    metadata: &'a EvmBatMetadata,
    contract_name: &str,
    modifier_name: &str,
) -> Option<(&'a ContractMetadata, crate::batbelt::evm::types::EvmModifierDef)> {
    let contract = metadata.get_contract_by_name(contract_name)?;
    if let Some(definition) = contract.modifiers.iter().find(|m| m.name == modifier_name) {
        return Some((contract, definition.clone()));
    }
    for base in &contract.base_contracts {
        if let Some(found) = find_modifier(metadata, base, modifier_name) {
            return Some(found);
        }
    }
    None
}

/// Map a call site name to the contract and function it refers to.
///
/// Handles `helper(...)` (same contract or inherited), `Lib.fn(...)`,
/// `super.fn(...)` and `stateVar.fn(...)` where the variable's declared type is
/// an interface with a known implementation.
fn resolve_call<'a>(
    metadata: &'a EvmBatMetadata,
    caller_contract: &ContractMetadata,
    call_name: &str,
    options: &AutoDeployOptions,
) -> Option<(&'a ContractMetadata, FunctionMetadata)> {
    let keep = |contract: &ContractMetadata| options.include_external || !contract.external;

    let (target_name, method) = match call_name.split_once('.') {
        Some((target, method)) => (Some(target), method),
        None => (None, call_name),
    };

    let candidates: Vec<String> = match target_name {
        None => {
            let mut chain = vec![caller_contract.name.clone()];
            chain.extend(caller_contract.base_contracts.iter().cloned());
            chain
        }
        Some("super") | Some("this") => {
            let mut chain = caller_contract.base_contracts.clone();
            chain.push(caller_contract.name.clone());
            chain
        }
        Some(target) => {
            if metadata.get_contract_by_name(target).is_some() {
                // Library or contract called by name.
                vec![target.to_string()]
            } else if let Some(variable) = caller_contract
                .state_variables
                .iter()
                .find(|v| v.name == target)
            {
                // `oracle.quote(...)` — resolve the variable's interface type to
                // whatever contract implements it.
                implementations_of(metadata, &variable.type_name)
            } else {
                Vec::new()
            }
        }
    };

    for candidate in candidates {
        let Some(contract) = metadata.get_contract_by_name(&candidate) else {
            continue;
        };
        if contract.contract_type == EvmContractType::Interface {
            // An interface has no body worth screenshotting; jump to the impl.
            for implementation in implementations_of(metadata, &contract.name) {
                if let Some(target) = metadata.get_contract_by_name(&implementation) {
                    if !keep(target) {
                        continue;
                    }
                    if let Some(function) = find_function(metadata, &target.name, method) {
                        return Some((target, function));
                    }
                }
            }
            continue;
        }
        if !keep(contract) {
            continue;
        }
        if let Some(function) = find_function(metadata, &contract.name, method) {
            // Skip interface-only declarations with no body.
            return Some((contract, function));
        }
    }
    None
}

/// Contracts implementing `type_name`, which may be an interface name or a
/// concrete contract name.
fn implementations_of(metadata: &EvmBatMetadata, type_name: &str) -> Vec<String> {
    let clean = type_name.trim();
    if let Some(interface) = metadata.interfaces.iter().find(|i| i.name == clean) {
        if !interface.implemented_by.is_empty() {
            return interface.implemented_by.clone();
        }
    }
    // Fall back to any contract declaring it as a base.
    let derived: Vec<String> = metadata
        .contracts
        .iter()
        .filter(|c| c.base_contracts.iter().any(|b| b == clean))
        .map(|c| c.name.clone())
        .collect();
    if !derived.is_empty() {
        return derived;
    }
    vec![clean.to_string()]
}

fn read_slice(file_path: &str, start_line: usize, end_line: usize) -> Vec<String> {
    let content = std::fs::read_to_string(file_path).unwrap_or_default();
    let lines: Vec<&str> = content.lines().collect();
    let start = start_line.saturating_sub(1);
    let end = end_line.min(lines.len());
    if start >= end {
        return Vec::new();
    }
    lines[start..end].iter().map(|l| l.to_string()).collect()
}

/// Render every node and read back its pixel size, without uploading anything.
fn render_and_measure(nodes: &mut [GraphNode]) -> Result<()> {
    let destination = BatFolder::Figures
        .get_path(true)
        .change_context(EvmMiroError)?;

    for node in nodes.iter_mut() {
        let code = read_slice(&node.file_path, node.start_line, node.end_line);
        if code.is_empty() {
            continue;
        }

        let pretty_path = crate::batbelt::path::prettify_source_code_path(&node.file_path)
            .unwrap_or_else(|_| node.file_path.clone());
        let mut rendered = vec![format!("// {pretty_path}"), String::new()];
        rendered.extend(code.iter().cloned());

        // silicon subtracts the two header lines from the offset so the printed
        // line numbers still match the file.
        node.line_offset = node.start_line.saturating_sub(PATH_HEADER_LINES);

        // `.js` gives Solidity the best Dracula colors available in syntect.
        let file_name = format!("{}.js", node.id.replace([':', '.', '/'], "_"));
        let png_path = silicon::create_figure(
            &rendered.join("\n"),
            &destination,
            &file_name,
            node.line_offset,
            Some(node.font_size),
            true,
        );
        let (width, height) = image::image_dimensions(&png_path)
            .into_report()
            .change_context(EvmMiroError)
            .attach_printable_lazy(|| format!("cannot measure {png_path}"))?;

        if width > 8192 || height > 8192 {
            log::warn!(
                "{} renders to {}x{}, above Miro's 8192 px limit",
                node.label,
                width,
                height
            );
        }

        node.png_path = png_path;
        node.png_width = width;
        node.png_height = height;
        node.rendered_lines = rendered;
    }
    Ok(())
}

/// Compose the frame locally: the real screenshots at their real positions,
/// plus a marker on every connector anchor.
///
/// Miro exposes no way to download a rendered board — export jobs are
/// Enterprise-only and `board.picture` is just a generic icon — so this is how
/// the layout gets reviewed without a human taking a screenshot. It shows
/// sizing, spacing and exactly which line each anchor lands on. What it cannot
/// show is Miro's own elbow routing, which the board decides.
fn render_preview(
    nodes: &[GraphNode],
    edges: &[GraphEdge],
    anchors: &[RelativeAnchor],
    layout: &GraphLayout,
    path: &str,
) -> Result<()> {
    use image::{Rgba, RgbaImage};

    // Keep the canvas manageable for very wide frames.
    let scale = (2600.0 / layout.frame_width).min(1.0);
    let width = (layout.frame_width * scale).round().max(1.0) as u32;
    let height = (layout.frame_height * scale).round().max(1.0) as u32;
    let mut canvas = RgbaImage::from_pixel(width, height, Rgba([24, 25, 33, 255]));

    for node in nodes {
        let Some(placed) = layout.node(&node.id) else {
            continue;
        };
        if node.png_path.is_empty() {
            continue;
        }
        let Ok(screenshot) = image::open(&node.png_path) else {
            continue;
        };
        let target_width = (placed.width * scale).round().max(1.0) as u32;
        let target_height = (placed.height * scale).round().max(1.0) as u32;
        let resized = screenshot.resize_exact(
            target_width,
            target_height,
            image::imageops::FilterType::Triangle,
        );
        let left = ((placed.x - placed.width / 2.0) * scale).round() as i64;
        let top = ((placed.y - placed.height / 2.0) * scale).round() as i64;
        image::imageops::overlay(&mut canvas, &resized, left, top);
    }

    // Anchor points and the straight line between them. The real connector is
    // routed by Miro, so treat this as "where it starts and ends", not "how it
    // gets there".
    let by_id: HashMap<&str, &GraphNode> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    for (index, (edge, anchor)) in edges.iter().zip(anchors.iter()).enumerate() {
        let (Some(from), Some(to)) = (layout.node(&edge.from), layout.node(&edge.to)) else {
            continue;
        };
        let start = (
            ((from.x - from.width / 2.0 + from.width * anchor.x_fraction) * scale) as i64,
            ((from.y - from.height / 2.0 + from.height * anchor.y_fraction) * scale) as i64,
        );
        let callee_fraction = by_id
            .get(edge.to.as_str())
            .map(|node| {
                silicon::line_geometry(Some(node.font_size))
                    .line_center_fraction(SIGNATURE_LINE_INDEX, node.png_height)
            })
            .unwrap_or(0.5);
        let end = (
            ((to.x - to.width / 2.0) * scale) as i64,
            ((to.y - to.height / 2.0 + to.height * callee_fraction) * scale) as i64,
        );

        let hex = DEPTH_COLORS[from.layer % DEPTH_COLORS.len()];
        let color = parse_hex(hex);
        draw_line(&mut canvas, start, end, color);
        // The arrow head sits on the caller, so mark that end fatter.
        draw_disc(&mut canvas, start, 5, color);
        draw_disc(&mut canvas, end, 2, color);
        let _ = index;
    }

    if let Some(parent) = std::path::Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    canvas
        .save(path)
        .into_report()
        .change_context(EvmMiroError)
        .attach_printable_lazy(|| format!("cannot write the preview to {path}"))?;
    Ok(())
}

fn parse_hex(hex: &str) -> image::Rgba<u8> {
    let clean = hex.trim_start_matches('#');
    let value = u32::from_str_radix(clean, 16).unwrap_or(0xffffff);
    image::Rgba([
        ((value >> 16) & 0xff) as u8,
        ((value >> 8) & 0xff) as u8,
        (value & 0xff) as u8,
        255,
    ])
}

fn draw_line(
    canvas: &mut image::RgbaImage,
    from: (i64, i64),
    to: (i64, i64),
    color: image::Rgba<u8>,
) {
    // Bresenham, thick enough to stay visible once the canvas is scaled down.
    let (mut x, mut y) = from;
    let dx = (to.0 - x).abs();
    let dy = -(to.1 - y).abs();
    let step_x = if x < to.0 { 1 } else { -1 };
    let step_y = if y < to.1 { 1 } else { -1 };
    let mut error = dx + dy;
    loop {
        draw_disc(canvas, (x, y), 1, color);
        if x == to.0 && y == to.1 {
            break;
        }
        let double = 2 * error;
        if double >= dy {
            error += dy;
            x += step_x;
        }
        if double <= dx {
            error += dx;
            y += step_y;
        }
    }
}

fn draw_disc(canvas: &mut image::RgbaImage, center: (i64, i64), radius: i64, color: image::Rgba<u8>) {
    for offset_y in -radius..=radius {
        for offset_x in -radius..=radius {
            if offset_x * offset_x + offset_y * offset_y > radius * radius {
                continue;
            }
            let x = center.0 + offset_x;
            let y = center.1 + offset_y;
            if x >= 0 && y >= 0 && (x as u32) < canvas.width() && (y as u32) < canvas.height() {
                canvas.put_pixel(x as u32, y as u32, color);
            }
        }
    }
}

fn cleanup(nodes: &[GraphNode]) {
    for node in nodes {
        if !node.png_path.is_empty() {
            let _ = std::fs::remove_file(&node.png_path);
        }
    }
}

fn print_dry_run(
    nodes: &[GraphNode],
    edges: &[GraphEdge],
    anchors: &[RelativeAnchor],
    layout: &GraphLayout,
    (frame_x, frame_y): (f64, f64),
) {
    println!(
        "  frame {}x{} at ({}, {})",
        layout.frame_width.round(),
        layout.frame_height.round(),
        frame_x.round(),
        frame_y.round()
    );
    println!(
        "  {:<38} {:>5} {:>9} {:>9} {:>7} {:>7}  {}",
        "node", "layer", "x", "y", "w", "h", "png"
    );
    let mut placed: Vec<_> = layout.nodes.iter().collect();
    placed.sort_by_key(|node| (node.layer, node.y as i64));
    for node in placed {
        let source = nodes.iter().find(|n| n.id == node.id);
        println!(
            "  {:<38} {:>5} {:>9.0} {:>9.0} {:>7.0} {:>7.0}  {}",
            truncate(&source.map(|n| n.label.clone()).unwrap_or_default(), 38),
            node.layer,
            node.x,
            node.y,
            node.width,
            node.height,
            source
                .map(|n| format!("{}x{}", n.png_width, n.png_height))
                .unwrap_or_default()
        );
    }

    println!("  {} connector(s):", edges.len());
    for (edge, anchor) in edges.iter().zip(anchors.iter()) {
        let Some(caller) = nodes.iter().find(|n| n.id == edge.from) else {
            continue;
        };
        let callee_label = nodes
            .iter()
            .find(|n| n.id == edge.to)
            .map(|n| n.label.clone())
            .unwrap_or_default();
        println!(
            "    {:<34} L{:<5} → {:<34} start ({:.2}%, {:.2}%)",
            truncate(&caller.label, 34),
            caller.start_line + edge.line_in_slice - 1,
            truncate(&callee_label, 34),
            anchor.x_fraction * 100.0,
            anchor.y_fraction * 100.0
        );
    }
    if !layout.back_edges.is_empty() {
        println!(
            "  {} cycle(s) will be drawn dashed: {:?}",
            layout.back_edges.len(),
            layout.back_edges
        );
    }
}

fn truncate(text: &str, width: usize) -> String {
    if text.len() <= width {
        return text.to_string();
    }
    format!("{}…", &text[..width.saturating_sub(1)])
}
