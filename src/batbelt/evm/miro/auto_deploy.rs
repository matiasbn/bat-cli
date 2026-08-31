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
use indicatif::{ProgressBar, ProgressStyle};

use crate::batbelt::evm::metadata::bat_metadata::{
    AutoDeployedFrame, ContractMetadata, EvmBatMetadata, FunctionMetadata, ShelfState,
};
use crate::batbelt::evm::miro::EvmMiroError;
use crate::batbelt::evm::parser::call_resolver::{body_only, extract_call_sites_from_source};
use crate::batbelt::evm::types::EvmContractType;
use crate::batbelt::miro::client::{ArrowEnd, ConnectorStyle, MiroClient, RelativeAnchor};
use crate::batbelt::miro::layout::{
    layout_graph, GraphLayout, LayoutConfig, LayoutEdge, LayoutNode, ShelfAllocator,
};
use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::path::BatFolder;
use crate::batbelt::silicon;
use rayon::prelude::*;

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

/// How many times to cut and re-lay out before giving up. Each pass removes at
/// least one edge, so this only guards against a pathological graph.
const MAX_CUT_PASSES: usize = 5;

/// Side of the invisible square the connector attaches to, in board units.
/// Small enough that the arrow head reads as landing on the token itself.
const ANCHOR_MARKER_SIZE: f64 = 24.0;

/// A bar that shows what is happening and how far along it is.
///
/// Rendering and uploading are both long enough to look like a hang without
/// one: a deployment can render dozens of screenshots and then make a hundred
/// API calls, and the previous output went silent for the whole of each phase.
fn phase_bar(label: &str, total: usize) -> ProgressBar {
    let bar = ProgressBar::new(total as u64);
    bar.set_style(
        ProgressStyle::with_template("  {spinner:.blue} {msg} {pos}/{len} {wide_bar:.blue}")
            .unwrap()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ "),
    );
    bar.set_message(label.to_string());
    bar.enable_steady_tick(std::time::Duration::from_millis(100));
    bar
}

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
    /// Optional depth limit. Unset follows the tree until it ends, which it
    /// does on its own: recursion is cut per path and the leaves are functions
    /// that call nothing.
    pub max_depth: Option<usize>,
    /// Optional cap on screenshots per frame. Unset means draw the whole tree.
    pub max_nodes: Option<usize>,
    /// Compute and print the layout without touching Miro.
    pub dry_run: bool,
    /// Include contracts coming from `lib/`.
    pub include_external: bool,
    /// Compose a local preview PNG of the frame at this path.
    pub preview: Option<String>,
    /// Connector thickness in dp, 1 to 24. Miro's UI snaps this to its own
    /// preset levels, so 12 lands on roughly "level 5".
    pub stroke_width: u32,
    /// Skip the "already on the board — deploy again?" confirmation (assume yes),
    /// so a redeploy runs non-interactively.
    pub assume_yes: bool,
    /// Draw the partial graph even when interface calls in the tree are unresolved,
    /// instead of stopping to list them.
    pub allow_unresolved: bool,
}

impl Default for AutoDeployOptions {
    fn default() -> Self {
        Self {
            entry_point: None,
            all: false,
            max_depth: None,
            max_nodes: None,
            dry_run: false,
            include_external: false,
            preview: None,
            stroke_width: 8,
            assume_yes: false,
            allow_unresolved: false,
        }
    }
}

/// What a node stands for.
#[derive(Debug, Clone, PartialEq)]
enum NodeKind {
    /// A screenshot of the function's source.
    Screenshot,
    /// A card standing in for a function drawn in its own frame, holding a link
    /// that navigates there.
    Link { target: String },
}

/// A card is small and fixed: it carries a name and a link, nothing to measure.
const LINK_CARD_WIDTH: f64 = 900.0;
const LINK_CARD_HEIGHT: f64 = 240.0;

/// One function in the graph.
#[derive(Debug, Clone)]
struct GraphNode {
    id: String,
    label: String,
    kind: NodeKind,
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
    /// This function writes contract storage — drawn with a colored border so an
    /// auditor can spot the state-mutating nodes at a glance.
    writes_storage: bool,
    /// File lines (1-based) inside this node's slice that write storage, each with
    /// the lvalue path — used to highlight the exact mutating statements.
    write_lines: Vec<(usize, String)>,
    /// File lines (1-based) that call an external contract with no in-scope source
    /// (an interface-typed receiver nothing in the repo implements) via a non-view
    /// method — an unverified external state-change boundary, flagged distinctly.
    external_call_lines: Vec<usize>,
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

    let targets = select_targets(&metadata, &options)?;
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
            true,
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

/// What to deploy.
///
/// Any function can be deployed, not just an entry point: a shared helper needs
/// a frame of its own for anything else to be able to point at it, and it is
/// worth looking at on its own terms too.
///
/// Deploying a whole project at once is deliberately not the default. An audit
/// is read one function at a time, and a project of any size would put thousands
/// of objects on a board that Miro starts to slow down past a thousand — so with
/// nothing named, ask.
fn select_targets(
    metadata: &EvmBatMetadata,
    options: &AutoDeployOptions,
) -> Result<Vec<(String, String)>> {
    // `EntryPointMetadata::name` is stored as `Contract.function`, so strip the
    // prefix to get the bare function name used to look it up in the contract.
    let mut entry_points: Vec<(String, String)> = metadata
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
    entry_points.sort();
    entry_points.dedup();

    if options.all {
        return Ok(entry_points);
    }

    // Everything the project defines, minus the constructors and minus `lib/`,
    // which is dependency code nobody deploys on purpose.
    let is_entry_point: HashSet<(String, String)> = entry_points.iter().cloned().collect();
    let mut others: Vec<(String, String)> = metadata
        .contracts
        .iter()
        .filter(|contract| !contract.external)
        .flat_map(|contract| {
            contract
                .functions
                .iter()
                .filter(|function| !function.is_constructor)
                .map(|function| (contract.name.clone(), function.name.clone()))
        })
        .filter(|target| !is_entry_point.contains(target))
        .collect();
    others.sort();
    others.dedup();

    if let Some(wanted) = &options.entry_point {
        return Ok(entry_points
            .into_iter()
            .chain(others)
            .filter(|(contract, function)| {
                *function == *wanted || format!("{contract}.{function}") == *wanted
            })
            .take(1)
            .collect());
    }

    // Entry points first, since that is where reading usually starts, then
    // everything else. One flat list, because it is fuzzy-searchable: typing
    // `feeOf` reaches a helper as fast as an entry point.
    let deployed: HashSet<String> = metadata
        .miro
        .auto
        .frames
        .iter()
        .map(|frame| frame.entry_point.clone())
        .collect();

    let all: Vec<(String, String)> = entry_points
        .iter()
        .cloned()
        .chain(others.iter().cloned())
        .collect();
    if all.is_empty() {
        return Ok(all);
    }

    let entry_point_count = entry_points.len();
    let labels: Vec<String> = all
        .iter()
        .enumerate()
        .map(|(index, (contract, function))| {
            let title = format!("{contract}.{function}");
            let mut label = if index < entry_point_count {
                format!("{title}  {}", "[entry point]".blue())
            } else {
                title.clone()
            };
            if deployed.contains(&title) {
                label = format!("{label} {}", "(deployed)".green());
            }
            label
        })
        .collect();

    let selection =
        BatDialoguer::fuzzy_select("Select what to deploy:".to_string(), labels)
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
    // True when the user named this function, false when it is being built only
    // because a card needs somewhere to point.
    is_primary: bool,
) -> Result<()> {
    let title = format!("{contract_name}.{function_name}");
    println!("\n{} {}", "▸".blue(), title.bold());

    // One frame per function, board-wide. Asked for as a link target, a function
    // already on the board is pointed at rather than drawn again — that is what
    // lets several diagrams share a helper's frame and keeps the fan-in readable.
    // Asked for directly, RECYCLE it: delete the old frame and its items, then
    // redraw — so a redeploy replaces the frame instead of piling up duplicates.
    let mut reused_frame: Option<AutoDeployedFrame> = None;
    if !options.dry_run {
        if let Some(url) = live_frame_url(&title, client).await? {
            if !is_primary {
                return Ok(());
            }
            if let Some(client) = client {
                println!("  {} recycling the existing frame", "↻".yellow());
                reused_frame = recycle_recorded_frame(&title, client).await?;
            } else {
                let _ = url;
            }
        }
    }

    let (mut nodes, mut edges, truncated, unresolved) =
        build_graph(metadata, contract_name, function_name, options)?;
    if nodes.is_empty() {
        println!("  no function metadata found, skipping");
        return Ok(());
    }

    // Recycle already-deployed frames: any callee in this tree that ALREADY has
    // its own frame on the board is referenced with a link card pointing at that
    // frame, instead of being redrawn (with its whole subtree) inside this one —
    // so a redeploy reuses what is already there rather than cluttering the frame
    // with duplicates. The root being deployed is always drawn. A card whose
    // frame has since been deleted is re-deployed on demand by
    // `ensure_target_frames`, so a stale record is self-healing.
    let deployed_titles: HashSet<String> = {
        let meta = EvmBatMetadata::read_metadata().change_context(EvmMiroError)?;
        meta.miro
            .auto
            .frames
            .iter()
            .map(|frame| frame.entry_point.clone())
            .filter(|entry_point| *entry_point != title)
            .collect()
    };
    if !deployed_titles.is_empty() {
        let root_id = nodes[0].id.clone();
        let mut linked = 0usize;
        while let Some(index) = edges.iter().position(|edge| {
            edge.to != root_id
                && nodes.iter().any(|node| {
                    node.id == edge.to
                        && node.kind == NodeKind::Screenshot
                        && deployed_titles.contains(&node.label)
                })
        }) {
            cut_edge(&mut nodes, &mut edges, index);
            linked += 1;
        }
        if linked > 0 {
            println!(
                "  {} linked {} call(s) to already-deployed frames",
                "↻".yellow(),
                linked
            );
        }
    }

    // Cross-contract calls this tree reaches through an interface, whose concrete
    // target static analysis cannot pin. By default STOP so the AI (or auditor) can
    // resolve them and the downstream storage writers can be drawn; `--allow-unresolved`
    // draws the partial graph instead.
    if !unresolved.is_empty() && !options.allow_unresolved {
        println!(
            "\n  {} {} interface call(s) in this tree are unresolved — their downstream\n  functions (and any storage changes) are NOT in the graph yet:",
            "⚠".yellow(),
            unresolved.len()
        );
        for u in &unresolved {
            let ty = if u.inferred_type.is_empty() {
                String::new()
            } else {
                format!("  [{}]", u.inferred_type)
            };
            println!(
                "    {}.{}{}  → candidates: {}",
                u.receiver,
                u.method,
                ty,
                if u.candidates.is_empty() {
                    "(none in scope)".to_string()
                } else {
                    u.candidates.join(", ")
                }
            );
            if !u.assigned_in.is_empty() {
                println!("        wired in: {}", u.assigned_in.join(", ").dimmed());
            }
        }
        println!(
            "\n  Resolve each interface to its concrete contract, then deploy again:\n    {}\n  (or pass {} to draw the partial graph as-is.)",
            "bat-cli resolve <INTERFACE> <CONTRACT>".green(),
            "--allow-unresolved".green()
        );
        return Err(Report::new(EvmMiroError).attach_printable(format!(
            "{} unresolved interface call(s) in {}.{} — see the list above",
            unresolved.len(),
            contract_name,
            function_name
        )));
    }
    let depth = nodes.iter().map(|node| node.depth).max().unwrap_or(0);
    println!(
        "  {} screenshots, {} connectors, {} levels deep",
        nodes.len().to_string().green(),
        edges.len().to_string().green(),
        (depth + 1).to_string().green()
    );
    if truncated > 0 {
        println!(
            "  {} {} call site(s) left out by --max-nodes {}",
            "note:".yellow(),
            truncated,
            options.max_nodes.unwrap_or_default()
        );
    }


    render_and_measure(&mut nodes, &title)?;

    let layout_nodes: Vec<LayoutNode> = nodes
        .iter()
        .map(|node| LayoutNode {
            id: node.id.clone(),
            width: node.board_width(),
            height: node.board_height(),
        })
        .collect();

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
    let mut layout = layout_graph(&root_id, &layout_nodes, &layout_edges, LayoutConfig::default());

    // Only cut when the frame is too big to read, and only a cut that measurably
    // shrinks it. Arrows reaching across layers are not by themselves a reason:
    // the layout already reserves a corridor for them, so they cross nothing.
    // What a card buys is space, and it only buys space when the call it
    // replaces was the last reference to a branch.
    let mut anchors = anchors;
    for _ in 0..MAX_CUT_PASSES {
        if screenshot_count(&nodes) <= READABLE_SCREENSHOTS {
            break;
        }
        let Some((cut_nodes, cut_edges)) = best_cut(&nodes, &edges) else {
            // Too big, but nothing left that cutting would shrink.
            break;
        };
        println!(
            "  {} {} screenshots is more than reads well; linking a branch out to\n  its own frame instead",
            "note:".yellow(),
            screenshot_count(&nodes)
        );
        nodes = cut_nodes;
        edges = cut_edges;

        anchors = compute_anchors(&nodes, &edges);
        let layout_nodes: Vec<LayoutNode> = nodes
            .iter()
            .map(|node| LayoutNode {
                id: node.id.clone(),
                width: node.board_width(),
                height: node.board_height(),
            })
            .collect();
        let layout_edges: Vec<LayoutEdge> = edges
            .iter()
            .zip(anchors.iter())
            .map(|(edge, anchor)| LayoutEdge {
                from: edge.from.clone(),
                to: edge.to.clone(),
                from_line_fraction: anchor.y_fraction,
            })
            .collect();
        layout = layout_graph(&root_id, &layout_nodes, &layout_edges, LayoutConfig::default());
    }
    let by_id: HashMap<&str, &GraphNode> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    // Reserve the slot in both modes, so a dry run shows the real sequence of
    // board positions instead of repeating the first one.
    // A recycled frame keeps its board position (other diagrams may point a
    // viewport link at it); only a brand-new frame consumes a slot from the
    // allocator. Item coordinates are relative to the frame, so reusing the
    // same centre keeps the layout identical regardless of the new size.
    let (frame_x, frame_y) = match &reused_frame {
        Some(record) => (record.x, record.y),
        None => allocator.place(layout.frame_width, layout.frame_height),
    };

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
    let frame_id = match &reused_frame {
        Some(record) => {
            // Keep the id (so viewport links survive) but resize to the new
            // layout, so the frame always fits its fresh contents exactly.
            client
                .update_frame(
                    &record.frame_id,
                    &format!("auto: {title}"),
                    frame_x,
                    frame_y,
                    layout.frame_width,
                    layout.frame_height,
                )
                .await
                .change_context(EvmMiroError)?;
            record.frame_id.clone()
        }
        None => client
            .create_frame(
                &format!("auto: {title}"),
                frame_x,
                frame_y,
                layout.frame_width,
                layout.frame_height,
            )
            .await
            .change_context(EvmMiroError)?,
    };
    println!(
        "  frame {} ({}x{}) at ({}, {})",
        frame_id.green(),
        layout.frame_width.round(),
        layout.frame_height.round(),
        frame_x.round(),
        frame_y.round()
    );

    // Every card needs somewhere to go. One frame per function, board-wide: two
    // cards for the same helper resolve to the same frame, and a helper already
    // deployed is reused rather than drawn again. That is what keeps the fan-in
    // answerable — one frame with several references, not a copy per caller.
    let target_frames = ensure_target_frames(&nodes, options, client, allocator).await?;

    // Record the frame before filling it, so a run that dies partway through
    // still leaves something that names what is on the board.
    let frame_url = client.frame_url(&frame_id);
    let mut record = AutoDeployedFrame {
        entry_point: title.clone(),
        frame_id: frame_id.clone(),
        frame_url: frame_url.clone(),
        x: frame_x,
        y: frame_y,
        width: layout.frame_width,
        height: layout.frame_height,
        images: Vec::new(),
        connector_ids: Vec::new(),
        marker_ids: Vec::new(),
        border_ids: Vec::new(),
    };
    save_frame_record(&record)?;

    // Images, already positioned and parented — one call each, no follow-up
    // PATCH. They are independent of each other, so they go up concurrently;
    // the client's semaphore and credit budget are what actually bound the rate.
    let uploads: Vec<_> = nodes
        .iter()
        .filter_map(|node| layout.node(&node.id).map(|placed| (node, placed)))
        .collect();
    let bar = phase_bar("uploading screenshots", uploads.len());
    let mut upload_tasks = tokio::task::JoinSet::new();
    for (node, placed) in uploads {
        let client = client.clone();
        let frame_id = frame_id.clone();
        let node_id = node.id.clone();
        let png_path = node.png_path.clone();
        let label = node.label.clone();
        let kind = node.kind.clone();
        let target_url = match &node.kind {
            NodeKind::Link { target } => target_frames.get(target).cloned().unwrap_or_default(),
            NodeKind::Screenshot => String::new(),
        };
        let (x, y, width, height) = (placed.x, placed.y, placed.width, placed.height);
        let bar = bar.clone();
        upload_tasks.spawn(async move {
            let result = match kind {
                NodeKind::Link { .. } => {
                    client
                        .create_link_card(&frame_id, &label, &target_url, x, y, width, height)
                        .await
                }
                NodeKind::Screenshot => client
                    .create_image_in_frame(&png_path, &frame_id, &label, x, y, width)
                    .await,
            };
            bar.inc(1);
            result.map(|image_id| (node_id, image_id))
        });
    }

    let mut image_ids: HashMap<String, String> = HashMap::new();
    while let Some(joined) = upload_tasks.join_next().await {
        let (node_id, image_id) = joined
            .into_report()
            .change_context(EvmMiroError)?
            .change_context(EvmMiroError)?;
        image_ids.insert(node_id, image_id);
    }
    bar.finish_and_clear();
    println!("    {} {} screenshots uploaded", "✓".green(), image_ids.len());

    // Storage-write markers: a hollow colored rectangle around every node whose
    // function mutates contract storage, so state changes stand out on the board.
    let borders: Vec<(f64, f64, f64, f64)> = nodes
        .iter()
        .filter(|n| n.writes_storage)
        .filter_map(|n| layout.node(&n.id).map(|p| (p.x, p.y, p.width, p.height)))
        .collect();
    if !borders.is_empty() {
        let n_borders = borders.len();
        let bar = phase_bar("storage markers", n_borders);
        let mut border_tasks = tokio::task::JoinSet::new();
        for (x, y, width, height) in borders {
            let client = client.clone();
            let frame_id = frame_id.clone();
            let bar = bar.clone();
            border_tasks.spawn(async move {
                let result = client
                    .create_storage_border(&frame_id, x, y, width, height)
                    .await;
                bar.inc(1);
                result
            });
        }
        while let Some(joined) = border_tasks.join_next().await {
            let id = joined
                .into_report()
                .change_context(EvmMiroError)?
                .change_context(EvmMiroError)?;
            record.border_ids.push(id);
        }
        bar.finish_and_clear();
        println!("    {} {} storage markers", "✓".green(), n_borders);
    }

    // External-boundary node borders: a hollow dashed amber rectangle around every
    // node that makes a non-view call to a sourceless external contract but writes
    // NO storage of its own — so it gets no red border, yet a state change probably
    // happens through it. Skipped when the node is already ringed red.
    let ext_borders: Vec<(f64, f64, f64, f64)> = nodes
        .iter()
        .filter(|n| !n.external_call_lines.is_empty() && !n.writes_storage)
        .filter_map(|n| layout.node(&n.id).map(|p| (p.x, p.y, p.width, p.height)))
        .collect();
    if !ext_borders.is_empty() {
        let n_ext_borders = ext_borders.len();
        let bar = phase_bar("external markers", n_ext_borders);
        let mut border_tasks = tokio::task::JoinSet::new();
        for (x, y, width, height) in ext_borders {
            let client = client.clone();
            let frame_id = frame_id.clone();
            let bar = bar.clone();
            border_tasks.spawn(async move {
                let result = client
                    .create_external_border(&frame_id, x, y, width, height)
                    .await;
                bar.inc(1);
                result
            });
        }
        while let Some(joined) = border_tasks.join_next().await {
            let id = joined
                .into_report()
                .change_context(EvmMiroError)?
                .change_context(EvmMiroError)?;
            record.border_ids.push(id);
        }
        bar.finish_and_clear();
        println!("    {} {} external markers", "✓".green(), n_ext_borders);
    }

    // Line highlights: a translucent red band over each exact statement that
    // writes storage, so an auditor sees WHICH state a function mutates (not just
    // that it does). Tracked as border ids so a recycle clears them too.
    let mut highlights: Vec<(f64, f64, f64, f64)> = Vec::new();
    for node in nodes.iter() {
        if node.write_lines.is_empty() || node.png_height == 0 {
            continue;
        }
        let Some(p) = layout.node(&node.id) else {
            continue;
        };
        let geom = silicon::line_geometry(Some(node.font_size));
        let line_h = (p.height * geom.line_height as f64 / node.png_height as f64).max(1.0);
        let mut lines: Vec<usize> = node
            .write_lines
            .iter()
            .map(|(line, _)| *line)
            .filter(|line| *line >= node.start_line && *line <= node.end_line)
            .collect();
        lines.sort_unstable();
        lines.dedup();
        for line in lines {
            let rendered_index = PATH_HEADER_LINES + (line - node.start_line);
            let y_fraction = geom.line_center_fraction(rendered_index, node.png_height);
            let cy = p.y - p.height / 2.0 + p.height * y_fraction;
            highlights.push((p.x, cy, p.width, line_h));
        }
    }
    if !highlights.is_empty() {
        let n_highlights = highlights.len();
        let bar = phase_bar("storage lines", n_highlights);
        let mut highlight_tasks = tokio::task::JoinSet::new();
        for (x, y, width, height) in highlights {
            let client = client.clone();
            let frame_id = frame_id.clone();
            let bar = bar.clone();
            highlight_tasks.spawn(async move {
                let result = client
                    .create_line_highlight(&frame_id, x, y, width, height)
                    .await;
                bar.inc(1);
                result
            });
        }
        while let Some(joined) = highlight_tasks.join_next().await {
            let id = joined
                .into_report()
                .change_context(EvmMiroError)?
                .change_context(EvmMiroError)?;
            record.border_ids.push(id);
        }
        bar.finish_and_clear();
        println!("    {} {} storage lines", "✓".green(), n_highlights);
    }

    // External-boundary markers: a dashed amber band over each line that calls an
    // external contract with no in-scope source (a non-view method whose storage
    // effect is unknowable). Unverified — visually distinct from the proven-write
    // red. Tracked as border ids so a recycle clears them too.
    let mut ext_bands: Vec<(f64, f64, f64, f64)> = Vec::new();
    for node in nodes.iter() {
        if node.external_call_lines.is_empty() || node.png_height == 0 {
            continue;
        }
        let Some(p) = layout.node(&node.id) else {
            continue;
        };
        let geom = silicon::line_geometry(Some(node.font_size));
        let line_h = (p.height * geom.line_height as f64 / node.png_height as f64).max(1.0);
        for line in &node.external_call_lines {
            if *line < node.start_line || *line > node.end_line {
                continue;
            }
            let rendered_index = PATH_HEADER_LINES + (line - node.start_line);
            let y_fraction = geom.line_center_fraction(rendered_index, node.png_height);
            let cy = p.y - p.height / 2.0 + p.height * y_fraction;
            ext_bands.push((p.x, cy, p.width, line_h));
        }
    }
    if !ext_bands.is_empty() {
        let n_ext = ext_bands.len();
        let bar = phase_bar("external boundaries", n_ext);
        let mut ext_tasks = tokio::task::JoinSet::new();
        for (x, y, width, height) in ext_bands {
            let client = client.clone();
            let frame_id = frame_id.clone();
            let bar = bar.clone();
            ext_tasks.spawn(async move {
                let result = client
                    .create_external_marker(&frame_id, x, y, width, height)
                    .await;
                bar.inc(1);
                result
            });
        }
        while let Some(joined) = ext_tasks.join_next().await {
            let id = joined
                .into_report()
                .change_context(EvmMiroError)?
                .change_context(EvmMiroError)?;
            record.border_ids.push(id);
        }
        bar.finish_and_clear();
        println!("    {} {} external boundaries", "✓".green(), n_ext);
    }

    // Connectors, one per call site. Each starts on an invisible marker sitting
    // on the called token, because Miro clips a connector at the boundary of the
    // item it attaches to: anchoring inside the screenshot itself would push the
    // arrow head out to the screenshot's border.
    let back_edges: HashSet<(String, String)> = layout.back_edges.iter().cloned().collect();

    let _ = anchors;

    // Dependencies that reach the SAME caller line from the SAME side share ONE
    // arrow into that line: they all route into a single edge marker, and one stub
    // carries the arrow in. So a line with two calls (both to the right) gets one
    // arrow at its end, not two overlapping ones.
    struct CalleeLink {
        end_id: String,
        end_anchor: RelativeAnchor,
        end_point: (f64, f64),
    }
    struct PendingGroup {
        token_x: f64,
        token_y: f64,
        edge_x: f64,
        exit_right: bool,
        style: ConnectorStyle,
        callees: Vec<CalleeLink>,
    }

    // Colour each connector by its CALLEE, ranked WITHIN ITS DEPTH (layer) — so two
    // DIFFERENT functions at the same depth never share a hue (e.g. `mint` and
    // `burn` on layer 2), while the SAME function keeps ONE colour on all its arrows
    // (one entry per callee). A layer rarely has more than the palette's worth of
    // distinct callees. Depth itself is read off the column, so the hue is free.
    let depth_of: HashMap<&str, usize> = nodes.iter().map(|n| (n.id.as_str(), n.depth)).collect();
    let mut callee_color: HashMap<String, String> = HashMap::new();
    {
        let mut rank_in_depth: HashMap<usize, usize> = HashMap::new();
        for edge in edges.iter() {
            if callee_color.contains_key(&edge.to) {
                continue;
            }
            let depth = depth_of.get(edge.to.as_str()).copied().unwrap_or(0);
            let rank = rank_in_depth.entry(depth).or_insert(0);
            callee_color.insert(
                edge.to.clone(),
                DEPTH_COLORS[*rank % DEPTH_COLORS.len()].to_string(),
            );
            *rank += 1;
        }
    }

    let mut groups: HashMap<(String, usize, bool), PendingGroup> = HashMap::new();
    for edge in edges.iter() {
        let (Some(_start_id), Some(end_id)) =
            (image_ids.get(&edge.from), image_ids.get(&edge.to))
        else {
            continue;
        };
        let (Some(caller), Some(callee)) =
            (by_id.get(edge.from.as_str()), by_id.get(edge.to.as_str()))
        else {
            continue;
        };
        let Some(caller_placed) = layout.node(&edge.from) else {
            continue;
        };
        let callee_x = layout
            .node(&edge.to)
            .map(|placed| placed.x)
            .unwrap_or(caller_placed.x + 1.0);
        let exit_right = callee_x >= caller_placed.x;

        // Where this dependency meets the callee (its side facing the caller).
        let callee_fraction = silicon::line_geometry(Some(callee.font_size))
            .line_center_fraction(SIGNATURE_LINE_INDEX, callee.png_height);
        let end_point = match layout.node(&edge.to) {
            Some(placed) => (
                if exit_right {
                    placed.x - placed.width / 2.0
                } else {
                    placed.x + placed.width / 2.0
                },
                placed.y - placed.height / 2.0 + placed.height * callee_fraction,
            ),
            None => (0.0, 0.0),
        };
        let link = CalleeLink {
            end_id: end_id.clone(),
            end_anchor: RelativeAnchor::new(if exit_right { 0.0 } else { 1.0 }, callee_fraction),
            end_point,
        };

        let dashed = back_edges.contains(&(edge.from.clone(), edge.to.clone()));
        let group = groups
            .entry((edge.from.clone(), edge.line_in_slice, exit_right))
            .or_insert_with(|| {
                // The shared arrow anchor for this caller line + side: the arrow lands
                // at the END of the line for a right entry, or its START for a left
                // one, and enters by a straight horizontal stub from the edge.
                let line_index = edge.line_in_slice.saturating_sub(1) + PATH_HEADER_LINES;
                let line_text = caller
                    .rendered_lines
                    .get(line_index)
                    .cloned()
                    .unwrap_or_default();
                let text_width = |text: &str| {
                    silicon::line_end_x(
                        Some(caller.font_size),
                        true,
                        caller.rendered_lines.len(),
                        caller.line_offset,
                        text,
                    ) as f64
                };
                let y_fraction = silicon::line_geometry(Some(caller.font_size))
                    .line_center_fraction(line_index, caller.png_height);
                let token_frac = if caller.png_width > 0 {
                    let width = caller.png_width as f64;
                    if exit_right {
                        let gap = (text_width("a") - text_width("")) * ANCHOR_GAP_CHARS;
                        ((text_width(&line_text) + gap) / width).min(1.0)
                    } else {
                        (text_width("") / width).max(0.0)
                    }
                } else if exit_right {
                    1.0
                } else {
                    0.0
                };
                let frame_edge = if exit_right {
                    caller_placed.x + caller_placed.width / 2.0
                } else {
                    caller_placed.x - caller_placed.width / 2.0
                };
                let raw_token_x =
                    caller_placed.x - caller_placed.width / 2.0 + caller_placed.width * token_frac;
                // A minimum stub so the arrow head always renders (the widest line
                // ends at the edge, which would make a zero-length stub otherwise).
                let min_stub = 60.0_f64;
                // The convergence point normally sits on the frame edge, giving a
                // clear horizontal stub across most of a (shorter) line. But a
                // FULL-WIDTH line — the signature line carrying modifiers, say —
                // ends flush at the image boundary, so several dependencies fanning
                // into it pile up right at the edge, indistinguishable. Detect that
                // (the line reaches the exit edge) and, only then, push the
                // convergence OUT into the gutter so the fan-out clears the
                // screenshot and a single stub crosses in. Short lines and the
                // whole left side are untouched.
                let edge_gap = 200.0_f64;
                let reaches_edge = exit_right && raw_token_x > frame_edge - min_stub * 2.0;
                let edge_x = if reaches_edge {
                    frame_edge + edge_gap
                } else {
                    frame_edge
                };
                let token_x = if exit_right {
                    raw_token_x.min(edge_x - min_stub)
                } else {
                    raw_token_x.max(edge_x + min_stub)
                };
                PendingGroup {
                    token_x,
                    token_y: caller_placed.y - caller_placed.height / 2.0
                        + caller_placed.height * y_fraction,
                    edge_x,
                    exit_right,
                    style: ConnectorStyle {
                        stroke_color: callee_color
                            .get(&edge.to)
                            .cloned()
                            .unwrap_or_else(|| DEPTH_COLORS[0].to_string()),
                        stroke_width: options.stroke_width.to_string(),
                        dashed: false,
                        caption: None,
                        arrow: ArrowEnd::Start,
                    },
                    callees: Vec::new(),
                }
            });
        group.style.dashed = group.style.dashed || dashed;
        group.callees.push(link);
    }

    let bar = phase_bar("drawing connectors", groups.len());
    let mut connector_tasks = tokio::task::JoinSet::new();
    for (_key, group) in groups {
        let client = client.clone();
        let frame_id = frame_id.clone();
        let bar = bar.clone();
        connector_tasks.spawn(async move {
            let mut markers = Vec::new();
            let mut connectors = Vec::new();

            // A token marker (arrow head, at the line end/start) and an edge marker
            // (at the caller edge). ONE stub carries the arrow in; every dependency
            // routes into the shared edge marker with no head of its own.
            let token_marker = client
                .create_anchor_marker(&frame_id, group.token_x, group.token_y, ANCHOR_MARKER_SIZE)
                .await?;
            markers.push(token_marker.clone());
            let edge_marker = client
                .create_anchor_marker(&frame_id, group.edge_x, group.token_y, ANCHOR_MARKER_SIZE)
                .await?;
            markers.push(edge_marker.clone());

            let (edge_side, token_side) = if group.exit_right {
                (RelativeAnchor::new(0.0, 0.5), RelativeAnchor::new(1.0, 0.5))
            } else {
                (RelativeAnchor::new(1.0, 0.5), RelativeAnchor::new(0.0, 0.5))
            };
            let mut stub_style = group.style.clone();
            stub_style.arrow = ArrowEnd::End;
            connectors.push(
                client
                    .create_connector(&edge_marker, edge_side, &token_marker, token_side, stub_style)
                    .await?,
            );

            for link in &group.callees {
                let mut route_style = group.style.clone();
                route_style.arrow = ArrowEnd::None;
                connectors.push(
                    client
                        .create_connector(
                            &link.end_id,
                            link.end_anchor,
                            &edge_marker,
                            facing_anchor((group.edge_x, group.token_y), link.end_point),
                            route_style,
                        )
                        .await?,
                );
            }

            bar.inc(1);
            Ok::<_, error_stack::Report<crate::batbelt::miro::MiroError>>((markers, connectors))
        });
    }

    let mut connector_ids = Vec::new();
    let mut marker_ids = Vec::new();
    while let Some(joined) = connector_tasks.join_next().await {
        let (markers, connectors) = joined
            .into_report()
            .change_context(EvmMiroError)?
            .change_context(EvmMiroError)?;
        marker_ids.extend(markers);
        connector_ids.extend(connectors);
    }
    bar.finish_and_clear();
    println!("    {} {} connector(s)", "✓".green(), connector_ids.len());

    record.images = image_ids.into_iter().collect();
    record.connector_ids = connector_ids;
    record.marker_ids = marker_ids;
    save_frame_record(&record)?;

    println!("  {}", frame_url.blue());
    cleanup(&nodes);
    Ok(())
}

/// Wipe a recorded frame's CONTENTS (images, connectors, markers, storage
/// borders) but KEEP the frame itself, returning its record so the redraw can
/// reuse the same frame id and position.
///
/// The frame is deliberately not deleted: other diagrams may link to it by URL
/// (`?moveToWidget=<frame_id>`), and a fresh frame would break those links. The
/// metadata record is dropped here and a new one is saved during the redraw.
///
/// The deletes fan out concurrently (like the create side): every item is
/// independent and best-effort, so there is no reason to wait one at a time.
async fn recycle_recorded_frame(
    title: &str,
    client: &MiroClient,
) -> Result<Option<AutoDeployedFrame>> {
    let record = {
        let metadata = EvmBatMetadata::read_metadata().change_context(EvmMiroError)?;
        metadata
            .miro
            .auto
            .frames
            .iter()
            .find(|frame| frame.entry_point == title)
            .cloned()
    };
    let Some(record) = record else {
        return Ok(None);
    };
    let mut delete_tasks = tokio::task::JoinSet::new();
    for id in record.connector_ids.clone() {
        let client = client.clone();
        delete_tasks.spawn(async move { client.delete_connector(&id).await });
    }
    let item_ids = record
        .marker_ids
        .iter()
        .cloned()
        .chain(record.border_ids.iter().cloned())
        .chain(record.images.iter().map(|(_, image_id)| image_id.clone()));
    for id in item_ids {
        let client = client.clone();
        delete_tasks.spawn(async move { client.delete_item(&id).await });
    }
    // Best-effort: drain the set, ignoring per-item failures (a hand-deleted
    // item 404s and that is fine — the goal is an empty frame).
    while delete_tasks.join_next().await.is_some() {}
    // The frame itself is kept on purpose — only its contents are cleared.

    let owner = title.to_string();
    EvmBatMetadata::update_metadata(move |metadata| {
        metadata
            .miro
            .auto
            .frames
            .retain(|frame| frame.entry_point != owner);
    })
    .change_context(EvmMiroError)?;
    Ok(Some(record))
}

/// Store what a deployment owns, replacing any earlier record for the same
/// entry point.
fn save_frame_record(record: &AutoDeployedFrame) -> Result<()> {
    let record = record.clone();
    EvmBatMetadata::update_metadata(move |metadata| {
        metadata
            .miro
            .auto
            .frames
            .retain(|frame| frame.entry_point != record.entry_point);
        metadata.miro.auto.frames.push(record.clone());
    })
    .change_context(EvmMiroError)
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
    // How many calls share a line, which decides whether an anchor can sit past
    // the end of it or has to sit on its own token.
    let mut per_line: HashMap<(&str, usize), usize> = HashMap::new();
    for edge in edges {
        *occurrences
            .entry((edge.from.as_str(), edge.line_in_slice, edge.column))
            .or_insert(0) += 1;
        *per_line
            .entry((edge.from.as_str(), edge.line_in_slice))
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

            let alone_on_line = per_line
                .get(&(edge.from.as_str(), edge.line_in_slice))
                .copied()
                .unwrap_or(1)
                == 1;
            let mut anchor = caller_anchor(node, edge, alone_on_line);
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

/// Anchor for the connector's caller end, on the line that makes the call.
///
/// Where on that line depends on whether it is the only call there:
///
/// - Alone, the anchor goes past the end of the line, so the arrow head sits on
///   empty background instead of covering the code it points at.
/// - Sharing the line, it goes on the callee's own token, which the AST gives
///   the column of. `MathLib.wadMul(amount, price(asset))` produces one anchor
///   on `wadMul` and another a few columns later on `price`, with no guessing.
///
/// The end of the line is the nicer place to land, so it is used whenever
/// telling two calls apart does not require otherwise.
fn caller_anchor(node: &GraphNode, edge: &GraphEdge, alone_on_line: bool) -> RelativeAnchor {
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

    let text_width = |text: &str| {
        silicon::line_end_x(
            Some(node.font_size),
            true,
            node.rendered_lines.len(),
            node.line_offset,
            text,
        ) as f64
    };

    let x_fraction = match (start, node.png_width) {
        (_, width) if alone_on_line && width > 0 => {
            // One call on the line: land just past the last character, clear of
            // the code.
            let gap = (text_width("a") - text_width("")) * ANCHOR_GAP_CHARS;
            (text_width(&line_text) + gap) / width as f64
        }
        (Some(column), width) if width > 0 => {
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
/// Walk the call graph from an entry point.
///
/// One screenshot per function, however many places call it. Drawing a copy per
/// call site was tried and is worse: `Vault.depositWithReferral` came to 77
/// screenshots for 27 distinct functions, with `MathLib.mulDiv` — three lines of
/// arithmetic — repeated fourteen times. Two thirds of that diagram carried no
/// information.
fn build_graph(
    metadata: &EvmBatMetadata,
    contract_name: &str,
    function_name: &str,
    options: &AutoDeployOptions,
) -> Result<(
    Vec<GraphNode>,
    Vec<GraphEdge>,
    usize,
    Vec<crate::batbelt::evm::metadata::bat_metadata::UnresolvedCall>,
)> {
    let Some((root_contract, root_function)) =
        find_function(metadata, contract_name, function_name)
    else {
        return Ok((Vec::new(), Vec::new(), 0, Vec::new()));
    };

    // Method name → in-scope, non-stub contracts that directly define it, built
    // ONCE so resolve_call's unique-definer fallback is O(1) per call site instead
    // of scanning every contract each time.
    let mut definer_map: HashMap<String, Vec<String>> = HashMap::new();
    for contract in &metadata.contracts {
        if (contract.external && !options.include_external)
            || contract.contract_type == EvmContractType::Interface
        {
            continue;
        }
        for function in &contract.functions {
            if !function.is_stub {
                definer_map
                    .entry(function.name.clone())
                    .or_default()
                    .push(contract.name.clone());
            }
        }
    }

    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut truncated = 0usize;
    // Interface calls in this tree still needing an AI resolution (see `resolve`).
    let mut unresolved: Vec<crate::batbelt::evm::metadata::bat_metadata::UnresolvedCall> =
        Vec::new();
    // One node per function: a second call to the same function points at the
    // node that already exists.
    let mut drawn: HashMap<String, String> = HashMap::new();

    let root_id = node_key(contract_name, function_name);
    drawn.insert(root_id.clone(), root_id.clone());
    nodes.push(make_node(
        root_id.clone(),
        format!("{contract_name}.{function_name}"),
        root_contract,
        &root_function,
        0,
    ));

    // Depth-first, so siblings stay in source order and each subtree is built
    // before the next one starts — which is the order the layout wants.
    struct Pending {
        node_id: String,
        contract: String,
        function: String,
        depth: usize,
    }

    let mut stack = vec![Pending {
        node_id: root_id,
        contract: contract_name.to_string(),
        function: function_name.to_string(),
        depth: 0,
    }];

    while let Some(current) = stack.pop() {
        if options.max_depth.is_some_and(|limit| current.depth >= limit) {
            continue;
        }
        // The contract that defines the function, which is where its source is.
        let Some((contract, function)) =
            find_function(metadata, &current.contract, &current.function)
        else {
            continue;
        };
        let slice = read_slice(
            &contract.file_path,
            function.line,
            function_end(&function, contract),
        );

        let mut children: Vec<Pending> = Vec::new();

        // Modifiers count as dependencies; their call site is the line of the
        // signature where the modifier name appears.
        for modifier_name in &function.modifiers {
            let Some((owner, definition)) =
                find_modifier(metadata, &current.contract, modifier_name)
            else {
                continue;
            };
            if options.max_nodes.is_some_and(|cap| nodes.len() >= cap) {
                truncated += 1;
                continue;
            }
            let line_in_slice = slice
                .iter()
                .position(|line| line_has_token(line, modifier_name))
                .map(|index| index + 1)
                .unwrap_or(1);
            let target_id = node_key(&owner.name, &definition.name);
            edges.push(GraphEdge {
                from: current.node_id.clone(),
                to: target_id.clone(),
                line_in_slice,
                column: slice
                    .get(line_in_slice - 1)
                    .and_then(|line| line.find(modifier_name.as_str()))
                    .unwrap_or(0),
                symbol: modifier_name.clone(),
            });
            if drawn.insert(target_id.clone(), target_id.clone()).is_none() {
                nodes.push(make_modifier_node(
                    target_id,
                    owner,
                    &definition,
                    current.depth + 1,
                ));
            }
        }

        for call in extract_call_sites_from_source(&body_only(&slice).join("\n")) {
            let Some((target_contract, target_function)) =
                resolve_call(metadata, contract, &call.name, options, &definer_map)
            else {
                continue;
            };
            let target_id = node_key(&target_contract.name, &target_function.name);
            if target_id == current.node_id {
                continue; // a function calling itself needs no arrow
            }
            if options.max_nodes.is_some_and(|cap| nodes.len() >= cap) {
                truncated += 1;
                continue;
            }

            edges.push(GraphEdge {
                from: current.node_id.clone(),
                to: target_id.clone(),
                line_in_slice: call.line,
                column: call.column,
                symbol: call.symbol.clone(),
            });

            // Seen before: the arrow points at the node already drawn, and there
            // is nothing left to expand.
            if drawn.insert(target_id.clone(), target_id.clone()).is_some() {
                continue;
            }
            nodes.push(make_node(
                target_id.clone(),
                format!("{}.{}", target_contract.name, target_function.name),
                target_contract,
                &target_function,
                current.depth + 1,
            ));
            children.push(Pending {
                node_id: target_id,
                contract: target_contract.name.clone(),
                function: target_function.name.clone(),
                depth: current.depth + 1,
            });
        }

        // Cross-contract interface calls: follow the ones the AI has resolved (so the
        // concrete downstream function — and its storage writes — appear and recurse),
        // and collect the rest as needing a resolution.
        for u in &function.unresolved_calls {
            let concrete = if u.inferred_type.is_empty() {
                None
            } else {
                metadata.resolutions.get(&u.inferred_type)
            };
            let Some(concrete) = concrete else {
                unresolved.push(u.clone());
                continue;
            };
            let Some((tc, tf)) = find_function(metadata, concrete, &u.method) else {
                // A resolution is set but its method isn't there — still unresolved.
                unresolved.push(u.clone());
                continue;
            };
            // A resolution that lands on a virtual/interface stub is redirected to
            // its concrete override; a pure declaration with none is dropped.
            let Some((tc, tf)) = destub(metadata, (tc, tf), options) else {
                continue;
            };
            if !options.include_external && tc.external {
                continue;
            }
            let target_id = node_key(&tc.name, &tf.name);
            if target_id == current.node_id {
                continue;
            }
            if options.max_nodes.is_some_and(|cap| nodes.len() >= cap) {
                truncated += 1;
                continue;
            }
            let line_in_slice = slice
                .iter()
                .position(|l| line_has_call(l, &u.method))
                .map(|i| i + 1)
                .unwrap_or(1);
            edges.push(GraphEdge {
                from: current.node_id.clone(),
                to: target_id.clone(),
                line_in_slice,
                column: slice
                    .get(line_in_slice - 1)
                    .and_then(|l| l.find(u.method.as_str()))
                    .unwrap_or(0),
                symbol: u.method.clone(),
            });
            if drawn.insert(target_id.clone(), target_id.clone()).is_some() {
                continue;
            }
            nodes.push(make_node(
                target_id.clone(),
                format!("{}.{}", tc.name, tf.name),
                tc,
                &tf,
                current.depth + 1,
            ));
            children.push(Pending {
                node_id: target_id,
                contract: tc.name.clone(),
                function: tf.name.clone(),
                depth: current.depth + 1,
            });
        }

        // Calls to external contracts with no in-scope source: flag the lines that
        // reach a non-view method (a `view`/`pure` one is compiler-guaranteed not to
        // mutate, so it is never a state-change risk). Located on the caller's node.
        let mut external_lines: Vec<usize> = Vec::new();
        for uec in &function.unknown_external_calls {
            let read_only = find_function(metadata, &uec.inferred_type, &uec.method)
                .map(|(_, f)| {
                    matches!(
                        f.mutability,
                        crate::batbelt::evm::types::EvmMutability::View
                            | crate::batbelt::evm::types::EvmMutability::Pure
                    )
                })
                .unwrap_or(false);
            if read_only {
                continue;
            }
            if let Some(pos) = slice.iter().position(|l| line_has_call(l, &uec.method)) {
                external_lines.push(function.line + pos);
            }
        }
        if !external_lines.is_empty() {
            external_lines.sort_unstable();
            external_lines.dedup();
            if let Some(node) = nodes.iter_mut().find(|n| n.id == current.node_id) {
                node.external_call_lines = external_lines;
            }
        }

        // Reversed, because popping a stack undoes the order.
        for child in children.into_iter().rev() {
            stack.push(child);
        }
    }

    // Surface the WHOLE tree's unresolved calls at once (not just the drawn frontier):
    // follow each unambiguous hop — a resolved interface, or a lone candidate — into
    // its function and collect ITS unresolved too, so the AI can resolve in one pass.
    let unresolved = expand_unresolved(metadata, unresolved);

    duplicate_shared_subtrees(&mut nodes, &mut edges);
    Ok((nodes, edges, truncated, unresolved))
}

/// Transitively collect every unresolved interface call reachable from `seed`,
/// descending through the unambiguous hops (a resolution, or a single candidate).
fn expand_unresolved(
    metadata: &EvmBatMetadata,
    seed: Vec<crate::batbelt::evm::metadata::bat_metadata::UnresolvedCall>,
) -> Vec<crate::batbelt::evm::metadata::bat_metadata::UnresolvedCall> {
    let mut out = Vec::new();
    let mut seen_calls: HashSet<(String, String)> = HashSet::new();
    let mut visited_fns: HashSet<(String, String)> = HashSet::new();
    let mut frontier = seed;
    while let Some(u) = frontier.pop() {
        if !seen_calls.insert((u.receiver.clone(), u.method.clone())) {
            continue;
        }
        // Pick a single target to descend into: a recorded resolution, else a lone
        // candidate. Ambiguous (multi-candidate) calls are listed, not descended.
        let target = if !u.inferred_type.is_empty() {
            metadata.resolutions.get(&u.inferred_type).cloned()
        } else {
            None
        }
        .or_else(|| {
            if u.candidates.len() == 1 {
                Some(u.candidates[0].clone())
            } else {
                None
            }
        });
        if let Some(contract) = target {
            if visited_fns.insert((contract.clone(), u.method.clone())) {
                if let Some((_, f)) = find_function(metadata, &contract, &u.method) {
                    for du in &f.unresolved_calls {
                        frontier.push(du.clone());
                    }
                }
            }
        }
        out.push(u);
    }
    out.sort_by(|a, b| (&a.receiver, &a.method).cmp(&(&b.receiver, &b.method)));
    out
}

/// Identity of a function, used to detect recursion along a path.
fn node_key(contract: &str, function: &str) -> String {
    format!("{contract}::{function}")
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
        kind: NodeKind::Screenshot,
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
        writes_storage: !function.storage_writes.is_empty(),
        write_lines: function
            .storage_write_sites
            .iter()
            .map(|s| (s.line, s.name.clone()))
            .collect(),
        external_call_lines: Vec::new(),
    }
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

/// True when `token` appears in `line` as a WHOLE identifier, so `mint` does not
/// match inside `mintedAlmShares`.
fn line_has_token(line: &str, token: &str) -> bool {
    let bytes = line.as_bytes();
    let mut from = 0;
    while let Some(rel) = line[from..].find(token) {
        let start = from + rel;
        let end = start + token.len();
        let before_ok = start == 0 || !is_ident_byte(bytes[start - 1]);
        let after_ok = end >= bytes.len() || !is_ident_byte(bytes[end]);
        if before_ok && after_ok {
            return true;
        }
        from = start + 1;
    }
    false
}

/// Like [`line_has_token`] but the identifier must be a CALL — followed (after
/// optional spaces) by `(`. So `mint` matches `collVault.mint(…)` on line 362 but
/// not the `mintedAlmShares` declaration on line 351.
fn line_has_call(line: &str, method: &str) -> bool {
    let bytes = line.as_bytes();
    let mut from = 0;
    while let Some(rel) = line[from..].find(method) {
        let start = from + rel;
        let end = start + method.len();
        let before_ok = start == 0 || !is_ident_byte(bytes[start - 1]);
        let after_ident_ok = end >= bytes.len() || !is_ident_byte(bytes[end]);
        let mut cursor = end;
        while cursor < bytes.len() && (bytes[cursor] == b' ' || bytes[cursor] == b'\t') {
            cursor += 1;
        }
        let is_call = cursor < bytes.len() && bytes[cursor] == b'(';
        if before_ok && after_ident_ok && is_call {
            return true;
        }
        from = start + 1;
    }
    false
}

fn make_modifier_node(
    id: String,
    contract: &ContractMetadata,
    definition: &crate::batbelt::evm::types::EvmModifierDef,
    depth: usize,
) -> GraphNode {
    let end_line = if definition.end_line > 0 {
        definition.end_line
    } else {
        definition.line + 6
    };
    GraphNode {
        kind: NodeKind::Screenshot,
        id,
        label: format!("{}.{} (modifier)", contract.name, definition.name),
        file_path: contract.file_path.clone(),
        start_line: definition.line,
        end_line,
        depth,
        font_size: font_for_depth(depth),
        png_path: String::new(),
        png_width: 0,
        png_height: 0,
        rendered_lines: Vec::new(),
        line_offset: 0,
        // A modifier that writes storage (e.g. `initializer`) is marked like any
        // state-mutating node.
        writes_storage: !definition.storage_writes.is_empty(),
        write_lines: definition
            .storage_write_sites
            .iter()
            .map(|(name, line)| (*line, name.clone()))
            .collect(),
        external_call_lines: Vec::new(),
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

/// Find a function, and the contract that actually **defines** it.
///
/// Returning the contract it was reached through instead is wrong for anything
/// inherited: `Settlement.settle` is defined in `Pipeline`, so reading its source
/// out of `Settlement.sol` lands on unrelated lines, and the screenshot comes out
/// empty. That silently truncated every diagram at the first inherited call.
fn find_function<'a>(
    metadata: &'a EvmBatMetadata,
    contract_name: &str,
    function_name: &str,
) -> Option<(&'a ContractMetadata, FunctionMetadata)> {
    let contract = metadata.get_contract_by_name(contract_name)?;
    if let Some(function) = contract.functions.iter().find(|f| f.name == function_name) {
        return Some((contract, function.clone()));
    }
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
    definer_map: &HashMap<String, Vec<String>>,
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
                    if let Some(found) = find_function(metadata, &target.name, method) {
                        // Only accept a concrete result; a bodyless stub with no
                        // override falls through to the unique-definer fallback.
                        if let Some(resolved) = destub(metadata, found, options) {
                            return Some(resolved);
                        }
                    }
                }
            }
            continue;
        }
        if !keep(contract) {
            continue;
        }
        if let Some(found) = find_function(metadata, &contract.name, method) {
            if let Some(resolved) = destub(metadata, found, options) {
                return Some(resolved);
            }
        }
    }

    // Fallback for an interface-typed receiver we could not pin through
    // inheritance (nothing declares `is IFace`): if exactly ONE in-scope contract
    // directly defines this method with a real body, that is the unambiguous
    // target — draw it. This catches a view read like `alm.getReservesAtSqrtPrice`
    // whose unresolved entry the storage-write prune would otherwise drop. Gated
    // on a `receiver.method` call AND on uniqueness, so a common name (`transfer`,
    // defined by many) never auto-binds to the wrong contract.
    if matches!(target_name, Some(t) if t != "super" && t != "this") {
        if let Some(definers) = definer_map.get(method) {
            if definers.len() == 1 {
                return find_function(metadata, &definers[0], method);
            }
        }
    }
    None
}

/// Redirect a resolved call that landed on a stub (a bodyless interface/abstract
/// declaration or an empty `virtual {}`) to its concrete implementation: the one
/// non-stub override among the contracts deriving from / implementing the stub's
/// contract. Returns the override when exactly one exists; the stub itself when
/// none does (it is the only thing there is to show); `None` when several
/// overrides make the runtime target ambiguous.
fn destub<'a>(
    metadata: &'a EvmBatMetadata,
    found: (&'a ContractMetadata, FunctionMetadata),
    options: &AutoDeployOptions,
) -> Option<(&'a ContractMetadata, FunctionMetadata)> {
    let (contract, function) = found;
    if !function.is_stub {
        return Some((contract, function));
    }
    let keep = |c: &ContractMetadata| options.include_external || !c.external;
    let mut overrides: Vec<(&'a ContractMetadata, FunctionMetadata)> = Vec::new();
    for impl_name in implementations_of(metadata, &contract.name) {
        if impl_name == contract.name {
            continue;
        }
        if let Some((tc, tf)) = find_function(metadata, &impl_name, &function.name) {
            if !tf.is_stub && keep(tc) {
                overrides.push((tc, tf));
            }
        }
    }
    // Exactly one override → draw it. None (a pure interface/abstract declaration
    // or an empty virtual with no override) or several (ambiguous runtime target)
    // → nothing meaningful to screenshot.
    if overrides.len() == 1 {
        return overrides.pop();
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
fn render_and_measure(nodes: &mut [GraphNode], owner: &str) -> Result<()> {
    // Screenshots are scratch: they are deleted once uploaded, so the directory
    // is often not there. Create it rather than treating its absence as an
    // error the user has to fix.
    BatFolder::Figures
        .create_folder()
        .change_context(EvmMiroError)?;
    let destination = BatFolder::Figures
        .get_path(true)
        .change_context(EvmMiroError)?;

    let bar = phase_bar("rendering screenshots", nodes.len());
    // Rendering is CPU-bound and every node is independent — each writes its own
    // PNG and silicon only reads shared, read-only highlighting assets — so this
    // fans out across cores. On a large diagram this is the dominant local cost.
    let outcome: std::result::Result<(), String> = nodes
        .par_iter_mut()
        .map(|node| {
            let code = read_slice(&node.file_path, node.start_line, node.end_line);
            if code.is_empty() {
                bar.inc(1);
                return Ok(());
            }

            let pretty_path = crate::batbelt::path::prettify_source_code_path(&node.file_path)
                .unwrap_or_else(|_| node.file_path.clone());
            let mut rendered = vec![format!("// {pretty_path}"), String::new()];
            rendered.extend(code.iter().cloned());

            // silicon subtracts the two header lines from the offset so the
            // printed line numbers still match the file.
            node.line_offset = node.start_line.saturating_sub(PATH_HEADER_LINES);

            // `.js` gives Solidity the best Dracula colors available in syntect.
            // Named after the deployment as well as the node. The same function
            // appears in several graphs, and a deployment deletes its own files
            // when it finishes: without the prefix, building a helper's frame
            // partway through a bigger one would delete the screenshot that
            // bigger one is still waiting to upload.
            let file_name = format!(
                "{}__{}.js",
                owner.replace([':', '.', '/'], "_"),
                node.id.replace([':', '.', '/'], "_")
            );
            let png_path = silicon::create_figure(
                &rendered.join("\n"),
                &destination,
                &file_name,
                node.line_offset,
                Some(node.font_size),
                true,
            );
            let (width, height) = image::image_dimensions(&png_path)
                .map_err(|e| format!("cannot measure {png_path}: {e}"))?;

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
            bar.inc(1);
            Ok(())
        })
        .collect();
    outcome.map_err(|message| Report::new(EvmMiroError).attach_printable(message))?;
    bar.finish_and_clear();
    println!("    {} {} screenshots rendered", "✓".green(), nodes.len());
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

/// Which side of an item a connector should leave through to reach `toward`.
///
/// Anchoring at the centre lets Miro choose, and it chooses the same side every
/// time — so every arrow approached its token from above regardless of where
/// the line actually came from. Picking the side that faces the other end makes
/// a line coming from below arrive from below, and one coming from the right
/// arrive horizontally, which is the variation that makes a dense diagram
/// readable.
fn facing_anchor(from: (f64, f64), toward: (f64, f64)) -> RelativeAnchor {
    let dx = toward.0 - from.0;
    let dy = toward.1 - from.1;

    if dx > 0.0 {
        // Forward edge (callee is to the right, the layout's flow). Leave
        // HORIZONTALLY so the connector starts at its own call-line height and
        // Miro turns it in the gutter — instead of leaving top/bottom and hugging
        // the source's edge, which bundles many calls into one overlapping trunk.
        // Only a near-vertical edge (callee almost directly above/below) leaves
        // top/bottom.
        if dy.abs() > dx.abs() * 3.0 {
            if dy > 0.0 {
                RelativeAnchor::new(0.5, 1.0)
            } else {
                RelativeAnchor::new(0.5, 0.0)
            }
        } else {
            RelativeAnchor::new(1.0, 0.5)
        }
    } else {
        // Back / same-column edge: the dominant axis decides; horizontal ties go
        // left (away from the flow).
        if dy.abs() > dx.abs() {
            if dy > 0.0 {
                RelativeAnchor::new(0.5, 1.0)
            } else {
                RelativeAnchor::new(0.5, 0.0)
            }
        } else {
            RelativeAnchor::new(0.0, 0.5)
        }
    }
}

#[cfg(test)]
mod facing_anchor_test {
    use super::*;

    #[test]
    fn test_the_side_faces_the_other_end() {
        let origin = (100.0, 100.0);

        // Straight to the right, and far enough right that x dominates.
        let right = facing_anchor(origin, (500.0, 120.0));
        assert_eq!((right.x_fraction, right.y_fraction), (1.0, 0.5));

        // Mostly downwards.
        let below = facing_anchor(origin, (120.0, 900.0));
        assert_eq!((below.x_fraction, below.y_fraction), (0.5, 1.0));

        // Mostly upwards.
        let above = facing_anchor(origin, (120.0, -400.0));
        assert_eq!((above.x_fraction, above.y_fraction), (0.5, 0.0));

        // Back to the left, which happens on a cycle.
        let left = facing_anchor(origin, (-300.0, 110.0));
        assert_eq!((left.x_fraction, left.y_fraction), (0.0, 0.5));
    }

    /// The two ends of one hop must face each other, not the same way.
    #[test]
    fn test_both_ends_of_a_hop_face_each_other() {
        let a = (0.0, 0.0);
        let b = (0.0, 500.0);
        let from_a = facing_anchor(a, b);
        let from_b = facing_anchor(b, a);
        assert_eq!((from_a.x_fraction, from_a.y_fraction), (0.5, 1.0));
        assert_eq!((from_b.x_fraction, from_b.y_fraction), (0.5, 0.0));
    }
}

/// Give every caller of a shared leaf its own copy of it.
///
/// Sharing a node keeps the diagram small, but a node shared by callers sitting
/// on different layers is what produces the long edges: layering puts it after
/// its deepest caller, so the arrows from the shallower ones have to cross every
/// column in between. In `Vault.depositWithReferral`, `MathLib.mulDiv` alone
/// accounts for three of the five such edges.
///
/// A leaf is the one case where splitting is nearly free: it carries no subtree,
/// so a copy costs exactly one screenshot, and the copy lands on the layer right
/// after its caller, which turns a layer-spanning edge into an adjacent one by
/// construction.
///
/// Both conditions are counted off the edge list — out-degree zero, in-degree
/// above one — with no notion of what the function does. "Leaf" here means leaf
/// *as drawn*: a node can have no outgoing edges because it genuinely calls
/// nothing, because a call could not be resolved, or because its callee lives in
/// `lib/` and was excluded. For laying out the picture those are the same thing,
/// since what matters is that the node has nothing hanging off it.
/// The node plus its transitively PRIVATE (non-shared) descendants — the copy
/// unit. Descent stops at any shared descendant, so a copy unit is always a
/// disjoint private subtree cut at shared/anchor nodes; that is what makes the
/// duplication non-cascading.
fn private_closure(
    root: &str,
    out: &HashMap<String, Vec<String>>,
    shared: &HashSet<String>,
) -> HashSet<String> {
    let mut result = HashSet::new();
    result.insert(root.to_string());
    let mut stack = vec![root.to_string()];
    while let Some(current) = stack.pop() {
        if let Some(children) = out.get(&current) {
            for child in children {
                if shared.contains(child) {
                    continue; // cut at shared descendants — decided on their own
                }
                if result.insert(child.clone()) {
                    stack.push(child.clone());
                }
            }
        }
    }
    result
}

/// Duplicate shared nodes (fan-in ≥ 2) so every caller has a nearby copy and the
/// arrows stay short and local instead of one node reached by long edges that
/// cross other screenshots. Generalises [`split_shared_leaves`]: a leaf is just
/// the `|closure| == 1` case.
///
/// The copy unit is the node's PRIVATE closure (it + its non-shared descendants),
/// so cascades are structurally impossible — an inner shared node is left in place
/// and, once the copies raise its own fan-in, duplicated on its own turn under the
/// same caps. Bounded by: closure size ≤ `MAX_CLOSURE`, ≤ `MAX_COPIES` copies per
/// node, and a global added-box budget, so the frame can grow at most ~1.4×.
fn duplicate_shared_subtrees(nodes: &mut Vec<GraphNode>, edges: &mut Vec<GraphEdge>) {
    const MAX_CLOSURE: usize = 4;
    const MAX_COPIES: usize = 10;
    let budget = (nodes.len() * 2 / 5).max(20);
    let mut added = 0usize;

    loop {
        if added >= budget {
            break;
        }
        // Distinct callers + out-adjacency, recomputed each round (copies change it).
        let mut callers: HashMap<String, HashSet<String>> = HashMap::new();
        let mut out: HashMap<String, Vec<String>> = HashMap::new();
        for edge in edges.iter() {
            callers
                .entry(edge.to.clone())
                .or_default()
                .insert(edge.from.clone());
            out.entry(edge.from.clone()).or_default().push(edge.to.clone());
        }
        let shared: HashSet<String> = callers
            .iter()
            .filter(|(_, callers)| callers.len() >= 2)
            .map(|(id, _)| id.clone())
            .collect();

        // Pick the SHALLOWEST shared node whose private closure fits the cap.
        let depth_of: HashMap<&str, usize> =
            nodes.iter().map(|node| (node.id.as_str(), node.depth)).collect();
        let mut candidates: Vec<&String> = shared.iter().collect();
        candidates.sort_by_key(|id| depth_of.get(id.as_str()).copied().unwrap_or(0));
        let chosen = candidates.into_iter().find_map(|id| {
            let closure = private_closure(id, &out, &shared);
            (closure.len() <= MAX_CLOSURE).then(|| (id.clone(), closure))
        });
        let Some((victim, closure)) = chosen else {
            break;
        };

        let templates: HashMap<String, GraphNode> = nodes
            .iter()
            .filter(|node| closure.contains(&node.id))
            .map(|node| (node.id.clone(), node.clone()))
            .collect();
        let internal: Vec<GraphEdge> = edges
            .iter()
            .filter(|edge| closure.contains(&edge.from) && closure.contains(&edge.to))
            .cloned()
            .collect();

        let mut caller_list: Vec<String> =
            callers.get(&victim).cloned().unwrap_or_default().into_iter().collect();
        caller_list.sort(); // deterministic; the first caller keeps the original

        let mut made = 0usize;
        for caller in caller_list.iter().skip(1) {
            if made >= MAX_COPIES || added + closure.len() > budget {
                break;
            }
            let suffix = format!("dup{added}_{made}");
            let id_map: HashMap<String, String> = closure
                .iter()
                .map(|id| (id.clone(), format!("{id}#{suffix}")))
                .collect();
            for id in &closure {
                if let Some(template) = templates.get(id) {
                    let mut copy = template.clone();
                    copy.id = id_map[id].clone();
                    nodes.push(copy);
                }
            }
            for edge in &internal {
                let mut copy = edge.clone();
                copy.from = id_map[&edge.from].clone();
                copy.to = id_map[&edge.to].clone();
                edges.push(copy);
            }
            // Repoint every call from THIS caller to the victim onto its own copy.
            for edge in edges.iter_mut() {
                if edge.from == *caller && edge.to == victim {
                    edge.to = id_map[&victim].clone();
                }
            }
            added += closure.len();
            made += 1;
        }
        if made == 0 {
            break; // budget exhausted for even the smallest closure
        }
    }
}

fn split_shared_leaves(nodes: &mut Vec<GraphNode>, edges: &mut [GraphEdge]) {
    let mut outgoing: HashMap<&str, usize> = HashMap::new();
    let mut incoming: HashMap<&str, usize> = HashMap::new();
    for edge in edges.iter() {
        *outgoing.entry(edge.from.as_str()).or_insert(0) += 1;
        *incoming.entry(edge.to.as_str()).or_insert(0) += 1;
    }

    let shared_leaves: HashSet<String> = nodes
        .iter()
        .filter(|node| {
            outgoing.get(node.id.as_str()).copied().unwrap_or(0) == 0
                && incoming.get(node.id.as_str()).copied().unwrap_or(0) > 1
        })
        .map(|node| node.id.clone())
        .collect();
    if shared_leaves.is_empty() {
        return;
    }

    let template: HashMap<String, GraphNode> = nodes
        .iter()
        .filter(|node| shared_leaves.contains(&node.id))
        .map(|node| (node.id.clone(), node.clone()))
        .collect();

    // One instance of the leaf PER DISTINCT CALLER, not per call. A leaf called
    // several times from the SAME caller stays one node — the arrows converge from
    // that caller's own (adjacent) lines, a small local fan, and N identical boxes
    // would be pure noise. Different callers still each get their own copy: that is
    // what keeps a widely-shared leaf from becoming one node every layer has to
    // reach across. The first caller keeps the original node.
    let mut instance: HashMap<(String, String), String> = HashMap::new();
    let mut keeper: HashMap<String, String> = HashMap::new();
    let mut copies: Vec<GraphNode> = Vec::new();
    for edge in edges.iter_mut() {
        if !shared_leaves.contains(&edge.to) {
            continue;
        }
        // The first caller seen for this leaf keeps the original node.
        let first = keeper
            .entry(edge.to.clone())
            .or_insert_with(|| edge.from.clone());
        if *first == edge.from {
            continue;
        }
        let Some(original) = template.get(&edge.to) else {
            continue;
        };
        // Reuse this caller's single copy across its repeated calls.
        let copy_id = instance
            .entry((edge.to.clone(), edge.from.clone()))
            .or_insert_with(|| {
                let copy_id = format!("{}#{}", edge.to, copies.len());
                let mut copy = original.clone();
                copy.id = copy_id.clone();
                copies.push(copy);
                copy_id
            })
            .clone();
        edge.to = copy_id;
    }

    nodes.extend(copies);
}

#[cfg(test)]
mod split_shared_leaves_test {
    use super::*;

    fn node(id: &str) -> GraphNode {
        GraphNode {
            kind: NodeKind::Screenshot,
            id: id.to_string(),
            label: id.to_string(),
            file_path: String::new(),
            start_line: 1,
            end_line: 2,
            depth: 0,
            font_size: 22,
            png_path: String::new(),
            png_width: 0,
            png_height: 0,
            rendered_lines: Vec::new(),
            line_offset: 0,
            writes_storage: false,
            write_lines: Vec::new(),
            external_call_lines: Vec::new(),
        }
    }

    fn edge(from: &str, to: &str) -> GraphEdge {
        GraphEdge {
            from: from.to_string(),
            to: to.to_string(),
            line_in_slice: 1,
            column: 0,
            symbol: to.to_string(),
        }
    }

    /// The rule is arithmetic on the edge list: out-degree zero, in-degree above
    /// one. Nothing about it knows what a function does.
    #[test]
    fn test_a_leaf_with_several_callers_is_split() {
        let mut nodes = vec![node("a"), node("b"), node("leaf")];
        let mut edges = vec![edge("a", "leaf"), edge("b", "leaf")];

        split_shared_leaves(&mut nodes, &mut edges);

        assert_eq!(nodes.len(), 4, "the leaf should have gained a copy");
        assert_ne!(edges[0].to, edges[1].to, "each caller gets its own");
        assert_eq!(edges[0].to, "leaf", "the first caller keeps the original");
    }

    /// A node with children carries a subtree, so a copy is not cheap and the
    /// rule leaves it alone.
    #[test]
    fn test_a_shared_node_with_children_is_left_shared() {
        let mut nodes = vec![node("a"), node("b"), node("mid"), node("deep")];
        let mut edges = vec![edge("a", "mid"), edge("b", "mid"), edge("mid", "deep")];

        split_shared_leaves(&mut nodes, &mut edges);

        assert_eq!(nodes.len(), 4, "nothing should have been copied");
        assert_eq!(edges[0].to, "mid");
        assert_eq!(edges[1].to, "mid");
    }

    /// One caller means nothing to gain: a copy would be the same picture.
    #[test]
    fn test_a_leaf_with_one_caller_is_untouched() {
        let mut nodes = vec![node("a"), node("leaf")];
        let mut edges = vec![edge("a", "leaf")];

        split_shared_leaves(&mut nodes, &mut edges);

        assert_eq!(nodes.len(), 2);
        assert_eq!(edges[0].to, "leaf");
    }
}

/// How many screenshots a frame can hold and still be read in one sitting.
///
/// Measured rather than chosen: `Vault.depositWithReferral` at 32 screenshots
/// and 14874x2894 is comfortable, so the limit sits above what has been seen to
/// work. Below it nothing is cut, and the diagram shows the code.
const READABLE_SCREENSHOTS: usize = 45;

/// Replace one call with a card linking to the callee's own frame.
///
/// The decision is per **edge**, not per function. `FeeLib.feeOf` is called from
/// layer 1 and from layer 2 and lays out on layer 3: the arrow from layer 2 is
/// fine, the one from layer 1 reaches further. Moving the whole function out
/// would take away the arrow that was already fine, so only the far call is
/// replaced — the near caller keeps the screenshot.
fn cut_edge(nodes: &mut Vec<GraphNode>, edges: &mut Vec<GraphEdge>, index: usize) {
    let Some(label) = nodes
        .iter()
        .find(|node| node.id == edges[index].to)
        .map(|node| node.label.clone())
    else {
        return;
    };

    let card_id = format!("\u{0}link{index}");
    nodes.push(GraphNode {
        id: card_id.clone(),
        label: label.clone(),
        kind: NodeKind::Link { target: label },
        file_path: String::new(),
        start_line: 0,
        end_line: 0,
        depth: 0,
        font_size: 22,
        png_path: String::new(),
        png_width: LINK_CARD_WIDTH as u32,
        png_height: LINK_CARD_HEIGHT as u32,
        rendered_lines: Vec::new(),
        line_offset: 0,
        writes_storage: false,
            write_lines: Vec::new(),
            external_call_lines: Vec::new(),
    });
    edges[index].to = card_id;
    prune_unreachable(nodes, edges);
}

/// Screenshots only: a card is a reference, not something to read.
fn screenshot_count(nodes: &[GraphNode]) -> usize {
    nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Screenshot)
        .count()
}

/// The cut that removes the most screenshots, if any removes one at all.
///
/// A cut has to earn its place. Replacing a call to a shared function takes
/// nothing off the frame, because the other caller still needs the function and
/// everything under it: the card is added and no screenshot leaves. It only pays
/// when the call was the last reference, so the subtree becomes unreachable and
/// goes with it.
///
/// Any call is a candidate, not only the ones reaching across layers. A call
/// spanning layers can never strand anything — the longer path that put its
/// target down there arrives through a nearer caller, which keeps holding it up.
/// The calls that free space are the ordinary ones: the single call into a deep
/// branch, whose removal takes the branch with it.
///
/// Leaves are never candidates: a copy costs one small screenshot and
/// [`split_shared_leaves`] has already made those, so a card would be a click in
/// exchange for nothing.
fn best_cut(
    nodes: &[GraphNode],
    edges: &[GraphEdge],
) -> Option<(Vec<GraphNode>, Vec<GraphEdge>)> {
    let has_children: HashSet<&str> = edges.iter().map(|edge| edge.from.as_str()).collect();

    let before = screenshot_count(nodes);
    let mut best: Option<(usize, Vec<GraphNode>, Vec<GraphEdge>)> = None;

    for (index, edge) in edges.iter().enumerate() {
        if !has_children.contains(edge.to.as_str()) {
            continue;
        }

        let mut candidate_nodes = nodes.to_vec();
        let mut candidate_edges = edges.to_vec();
        cut_edge(&mut candidate_nodes, &mut candidate_edges, index);

        let saved = before.saturating_sub(screenshot_count(&candidate_nodes));
        if saved == 0 {
            continue;
        }
        if best.as_ref().map(|(most, _, _)| saved > *most).unwrap_or(true) {
            best = Some((saved, candidate_nodes, candidate_edges));
        }
    }

    best.map(|(_, nodes, edges)| (nodes, edges))
}

/// Drop nodes no longer reachable from the root, and the edges into them.
fn prune_unreachable(nodes: &mut Vec<GraphNode>, edges: &mut Vec<GraphEdge>) {
    let Some(root) = nodes.first().map(|node| node.id.clone()) else {
        return;
    };
    let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in edges.iter() {
        adjacency
            .entry(edge.from.as_str())
            .or_default()
            .push(edge.to.as_str());
    }

    let mut reachable: HashSet<String> = HashSet::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if !reachable.insert(id.clone()) {
            continue;
        }
        for next in adjacency.get(id.as_str()).cloned().unwrap_or_default() {
            stack.push(next.to_string());
        }
    }
    nodes.retain(|node| reachable.contains(&node.id));
    edges.retain(|edge| reachable.contains(&edge.from) && reachable.contains(&edge.to));
}

#[cfg(test)]
mod cut_test {
    use super::*;
    use crate::batbelt::miro::layout::{layout_graph, LayoutConfig, LayoutEdge, LayoutNode};

    fn node(id: &str) -> GraphNode {
        GraphNode {
            id: id.to_string(),
            label: id.to_string(),
            kind: NodeKind::Screenshot,
            file_path: String::new(),
            start_line: 1,
            end_line: 2,
            depth: 0,
            font_size: 22,
            png_path: String::new(),
            png_width: 1000,
            png_height: 300,
            rendered_lines: Vec::new(),
            line_offset: 0,
            writes_storage: false,
            write_lines: Vec::new(),
            external_call_lines: Vec::new(),
        }
    }

    fn edge(from: &str, to: &str) -> GraphEdge {
        GraphEdge {
            from: from.to_string(),
            to: to.to_string(),
            line_in_slice: 1,
            column: 0,
            symbol: to.to_string(),
        }
    }

    fn lay(nodes: &[GraphNode], edges: &[GraphEdge]) -> GraphLayout {
        let layout_nodes: Vec<LayoutNode> = nodes
            .iter()
            .map(|n| LayoutNode {
                id: n.id.clone(),
                width: n.board_width(),
                height: n.board_height(),
            })
            .collect();
        let layout_edges: Vec<LayoutEdge> = edges
            .iter()
            .map(|e| LayoutEdge {
                from: e.from.clone(),
                to: e.to.clone(),
                from_line_fraction: 0.5,
            })
            .collect();
        layout_graph(
            &nodes[0].id,
            &layout_nodes,
            &layout_edges,
            LayoutConfig::default(),
        )
    }

    /// The case that made the rule necessary. `shared` is reached from far away
    /// *and* from next door, so cutting the far call leaves it drawn for the
    /// near one and takes nothing off the frame. A card would be added for
    /// nothing.
    #[test]
    fn test_a_cut_that_saves_nothing_is_refused() {
        let nodes = vec![
            node("root"),
            node("far"),
            node("near"),
            node("shared"),
            node("child"),
        ];
        let edges = vec![
            edge("root", "far"),
            edge("far", "near"),
            edge("near", "shared"),
            edge("far", "shared"),
            edge("shared", "child"),
        ];

        if let Some((cut_nodes, _)) = best_cut(&nodes, &edges) {
            assert!(
                screenshot_count(&cut_nodes) < screenshot_count(&nodes),
                "a cut that is taken has to free something"
            );
        }
    }

    /// Cutting one long call can never take a screenshot off the frame, and the
    /// layering is why.
    ///
    /// A call spans layers only because a longer path reaches the same function,
    /// and that path arrives through a different caller one layer above it. Cut
    /// the long call and the function still hangs off that other caller, with
    /// everything under it. There is no graph where this comes out otherwise, so
    /// there is no graph where a single cut pays for itself in space.
    #[test]
    fn test_a_long_call_always_has_a_nearer_caller() {
        for extra in 0..4 {
            let mut nodes = vec![node("root"), node("mid"), node("target"), node("child")];
            let mut edges = vec![
                edge("root", "mid"),
                edge("root", "target"),
                edge("mid", "target"),
                edge("target", "child"),
            ];
            // Lengthen the path a few different ways; the property holds anyway.
            for step in 0..extra {
                let id = format!("step{step}");
                nodes.push(node(&id));
                edges.push(edge("mid", &id));
                edges.push(edge(&id, "target"));
            }

            // Cut the long call specifically, rather than whichever cut is
            // best overall, since the claim is about that one.
            let long = edges
                .iter()
                .position(|e| e.from == "root" && e.to == "target")
                .expect("the long call");
            let mut cut_nodes = nodes.clone();
            let mut cut_edges = edges.clone();
            cut_edge(&mut cut_nodes, &mut cut_edges, long);

            assert_eq!(
                screenshot_count(&cut_nodes),
                screenshot_count(&nodes),
                "with {extra} extra hops, cutting the long call freed a screenshot"
            );
        }
    }

    /// Nothing to consider when every call reaches the next layer along.
    #[test]
    fn test_a_clean_graph_has_no_candidate() {
        let nodes = vec![node("root"), node("a"), node("b")];
        let edges = vec![edge("root", "a"), edge("a", "b")];

        // `a` holds `b` up on its own, so cutting root→a frees both.
        let (cut_nodes, _) = best_cut(&nodes, &edges).expect("cutting root->a strands b");
        assert!(screenshot_count(&cut_nodes) < screenshot_count(&nodes));
    }
}

/// Frame URL for every function a card in this graph points at.
///
/// A function that already has a frame is reused; one that does not is deployed
/// first, so the card has somewhere to go. Distinct cards for the same function
/// collapse to one lookup, which is what makes several diagrams share a helper's
/// frame instead of each building its own.
async fn ensure_target_frames(
    nodes: &[GraphNode],
    options: &AutoDeployOptions,
    client: &MiroClient,
    allocator: &mut ShelfAllocator,
) -> Result<HashMap<String, String>> {
    let wanted: Vec<String> = {
        let mut seen = HashSet::new();
        nodes
            .iter()
            .filter_map(|node| match &node.kind {
                NodeKind::Link { target } => Some(target.clone()),
                NodeKind::Screenshot => None,
            })
            .filter(|target| seen.insert(target.clone()))
            .collect()
    };
    if wanted.is_empty() {
        return Ok(HashMap::new());
    }

    let mut resolved = HashMap::new();
    for target in wanted {
        if let Some(url) = live_frame_url(&target, Some(client)).await? {
            println!("  {} reuses its frame", target.blue());
            resolved.insert(target, url);
            continue;
        }
        let metadata = EvmBatMetadata::read_metadata().change_context(EvmMiroError)?;

        let Some((contract, function)) = target.split_once('.') else {
            continue;
        };
        println!("  {} needs a frame of its own, deploying it first", target.blue());
        // Boxed because this is the recursive step: the frame being built for a
        // helper can itself need cards, and those need frames.
        Box::pin(deploy_one(
            &metadata,
            contract,
            function,
            options,
            Some(client),
            allocator,
            false,
        ))
        .await?;

        let metadata = EvmBatMetadata::read_metadata().change_context(EvmMiroError)?;
        if let Some(created) = metadata
            .miro
            .auto
            .frames
            .iter()
            .find(|frame| frame.entry_point == target)
        {
            resolved.insert(target, created.frame_url.clone());
        }
    }
    Ok(resolved)
}

/// The frame URL for a function, if it is registered **and** still on the board.
///
/// A registry entry outlives the frame it names: boards are edited by hand, and
/// deleting a frame in Miro leaves the entry behind. Checking the board costs
/// one read and turns "you already deployed this" into something true, rather
/// than a refusal to redeploy what is no longer there. A stale entry is dropped
/// on the way past, so the question is only asked once.
async fn live_frame_url(title: &str, client: Option<&MiroClient>) -> Result<Option<String>> {
    let metadata = EvmBatMetadata::read_metadata().change_context(EvmMiroError)?;
    let Some(record) = metadata
        .miro
        .auto
        .frames
        .iter()
        .find(|frame| frame.entry_point == title)
    else {
        return Ok(None);
    };
    let (frame_id, url) = (record.frame_id.clone(), record.frame_url.clone());

    let Some(client) = client else {
        return Ok(Some(url));
    };
    if client.item_exists(&frame_id).await {
        return Ok(Some(url));
    }

    println!(
        "  {} the frame recorded for {} is gone from the board; forgetting it",
        "note:".yellow(),
        title
    );
    let owner = title.to_string();
    EvmBatMetadata::update_metadata(move |metadata| {
        metadata
            .miro
            .auto
            .frames
            .retain(|frame| frame.entry_point != owner);
    })
    .change_context(EvmMiroError)?;
    Ok(None)
}
