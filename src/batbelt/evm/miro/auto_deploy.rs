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
use crate::batbelt::miro::client::{
    ArrowEnd, ConnectorStroke, ConnectorStyle, MiroClient, RelativeAnchor,
};
use crate::batbelt::miro::layout::{
    layout_graph, GraphLayout, LayoutConfig, LayoutEdge, LayoutNode, ShelfAllocator,
};
use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::path::BatFolder;
use crate::batbelt::silicon::{self, TracedName};
use rand::seq::SliceRandom;
use rayon::prelude::*;

type Result<T> = error_stack::Result<T, EvmMiroError>;

/// Gap left between the end of the code and the connector anchor, in characters.
const ANCHOR_GAP_CHARS: f64 = 1.0;
/// Vertical distance from the top of the callee where its signature sits: line
/// index 2 of the image, because `include_path` prepends `// path` plus a blank.
pub(crate) const SIGNATURE_LINE_INDEX: usize = 2;
/// Number of lines `include_path` prepends to the rendered content.
pub(crate) const PATH_HEADER_LINES: usize = 2;
/// Vertical margin left between the existing board content and the region we
/// reserve for automatic deployments.
const REGION_MARGIN: f64 = 5_000.0;


/// Side of the invisible square the connector attaches to, in board units.
/// Small enough that the arrow head reads as landing on the token itself.
/// The invisible shape a connector endpoint anchors to, at Miro's smallest allowed size.
///
/// Miro clips a connector to the item's border, so the marker is a HOLE in the line: at 24
/// units the two halves visibly failed to meet, which read as two lines passing by rather
/// than one arrow arriving. 8 is the floor — 4 and below are refused with a 400 — and it
/// is no wider than the 8dp stroke, so the gap disappears under the line itself.
pub(crate) const ANCHOR_MARKER_SIZE: f64 = 8.0;
/// Horizontal distance between two arrows' vertical lanes in a gutter: five times the
/// default 8dp stroke, so two arrows at full width still have four strokes of white
/// between them. Narrowed automatically when a gutter cannot fit them all.
const LANE_PITCH: f64 = 40.0;
/// A frame's background when something in it changes state, and when something in it
/// only might — the same red and amber as the per-node markings, several steps lighter.
/// The job is to be legible from far enough out that a whole cluster fits on screen;
/// up close the screenshots are what is being read, and a strong tint behind them
/// competes with the code. Miro's own "light red" (`#ffc6c6`) is already too much.
const FRAME_FILL_WRITES: &str = "#fff0ef";
const FRAME_FILL_MAY_WRITE: &str = "#fff7ec";
/// How many far callers may each get their OWN copy of a callee before it is given a
/// frame instead. Copying is what removes a crossing arrow, and for a leaf it is
/// cheap — but the cost is paid once per caller, so the same rule that keeps `sqrt`
/// beside its two callers puts nine copies of a helper on a frame when nine callers
/// reach it. Past this, one drawing in a frame of its own and a card beside each
/// caller is both smaller and easier to read.
const MAX_COPIES_OF_ONE_CALLEE: usize = 3;

/// A bar that shows what is happening and how far along it is.
///
/// Rendering and uploading are both long enough to look like a hang without
/// one: a deployment can render dozens of screenshots and then make a hundred
/// API calls, and the previous output went silent for the whole of each phase.
pub(crate) fn phase_bar(label: &str, total: usize) -> ProgressBar {
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

/// The colours arrows are drawn in — one per callee, so two arrows a reader compares never
/// share a hue (`color_callees`). It is bat-cli's one palette, shared with the names traced
/// inside a screenshot, so a colour means the same kind of thing everywhere on the board.
const DEPTH_COLORS: &[&str] = crate::batbelt::silicon::BAT_PALETTE;

#[derive(Debug, Clone)]
pub struct AutoDeployOptions {
    /// Deploy only this entry point (`name` or `Contract.name`).
    pub entry_point: Option<String>,
    /// Compute and print the layout without touching Miro.
    pub dry_run: bool,
    /// Extend each screenshot upward to include the function's NatSpec block, so
    /// the diagram carries the documented intent next to the code.
    pub with_documentation: bool,
    /// Compose a local preview PNG of the frame at this path.
    pub preview: Option<String>,
    /// `effects` only: also put the report on the board, in a frame of its own.
    pub deploy_effects: bool,
    /// Connector thickness in dp, 1 to 24. Miro's UI snaps this to its own
    /// preset levels, so 12 lands on roughly "level 5".
    pub stroke_width: u32,
    /// Draw the partial graph even when interface calls in the tree are unresolved,
    /// instead of stopping to list them.
    pub allow_unresolved: bool,
    /// Answer the "this entry point already has a deployment — deploy again?" question
    /// with yes, so a re-deploy runs unattended.
    pub assume_yes: bool,
    /// Contracts whose functions are never drawn: a name (`Math`) or any part of a
    /// path (`openzeppelin-contracts/contracts/utils/math`). The call is still shown
    /// in the caller's screenshot — it is the callee's box that is left out.
    pub ignore_contracts: Vec<String>,
    /// Draw the ENTIRE call graph inline in one frame: no branch is cut to its own
    /// frame, and no already-deployed frame is linked — every function is a
    /// screenshot. Lets you see how big a large function is with screenshots only
    /// (and how Miro copes), and gives a step-through-able single frame.
    pub inline_all: bool,
}

impl Default for AutoDeployOptions {
    fn default() -> Self {
        Self {
            entry_point: None,
            dry_run: false,
            with_documentation: false,
            preview: None,
            deploy_effects: false,
            stroke_width: 8,
            allow_unresolved: false,
            assume_yes: false,
            ignore_contracts: Vec::new(),
            inline_all: false,
        }
    }
}

/// Shared context for a deploy: the entry point that owns the whole cluster, and
/// the ids of the PREVIOUS cluster's frames, so a still-live old frame is never
/// reused (every deploy is fresh) and can be reported for manual deletion
/// afterwards. Threaded through the recursive deploy.


/// What a node stands for.
#[derive(Debug, Clone, PartialEq)]
enum NodeKind {
    /// A screenshot of the function's source.
    Screenshot,
    /// A card standing in for a function drawn in its own frame, holding a link
    /// that navigates there.
    Link {
        target: String,
        /// File of the contract the card stands for, so the frame deployed for it is the
        /// same copy that was drawn here and not another contract with the same name.
        file: String,
    },
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
    /// This function writes no storage itself, but something it calls does.
    ///
    /// Without this a pass-through reads as inert: `DebtToken.mint` exists only to reach
    /// `ERC20._update`, and in OpenZeppelin v5 neither `mint` nor `_mint` assigns anything
    /// — the write is three hops down. The chain was drawn correctly and looked like it
    /// changed nothing.
    leads_to_write: bool,
    /// Call lines (1-based, file) that reach a storage write, as
    /// `(line, symbol, callee node id)`. The id is empty when the callee never entered
    /// the graph at all.
    ///
    /// These are CANDIDATES. One state change must produce exactly ONE mark, so a line is
    /// only drawn when its callee is not itself a screenshot on this frame — otherwise the
    /// callee carries the mark, nearer to where the write really happens. The filter runs
    /// at draw time because framing can turn a screenshot into a link card after the graph
    /// is built.
    write_call_lines: Vec<(usize, String, String)>,
    /// Call sites whose callee REACHES a boundary — a low-level call or a method on a
    /// contract with no source here — without writing storage this scan can see. Same
    /// shape, and the same rule, as `write_call_lines`: the deepest frame that draws the
    /// boundary owns the mark, and a frame that stops short of it carries the mark on the
    /// call instead. Without it a token transfer four hops down was marked nowhere at all.
    external_call_sites: Vec<(usize, String, String)>,
    /// Display scale for this placement. Every function is rendered ONCE at the
    /// reference font (`REFERENCE_FONT`, the depth-0 size); a deeper node shows the
    /// same image shrunk by this factor (< 1), so the source is rendered once and
    /// reused across depths and duplicate placements instead of re-rendered. Line
    /// fractions are scale-invariant, so only the board size and upload width use it.
    scale: f64,
}

impl GraphNode {
    fn board_width(&self) -> f64 {
        self.png_width as f64 * BOARD_UNITS_PER_PIXEL * self.scale
    }

    fn board_height(&self) -> f64 {
        self.png_height as f64 * BOARD_UNITS_PER_PIXEL * self.scale
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
pub(crate) const BOARD_UNITS_PER_PIXEL: f64 = 1.0;

/// Font per depth: the entry point is rendered biggest and leaves smallest, so a
/// deep graph stays readable. Width now follows from the code itself.
/// The one font every screenshot is rendered at (the largest, depth-0 size). A
/// deeper node reuses that render shrunk via `scale_for_depth`, so a function is
/// rendered once and reused across depths and duplicate placements.
pub(crate) const REFERENCE_FONT: usize = 32;

fn font_for_depth(depth: usize) -> usize {
    match depth {
        0 => 32,
        1 => 26,
        _ => 22,
    }
}

/// How much to shrink a node's (reference-font) render for its depth. Always ≤ 1,
/// so text is only ever scaled DOWN and stays crisp.
fn scale_for_depth(depth: usize) -> f64 {
    font_for_depth(depth) as f64 / REFERENCE_FONT as f64
}


/// Print everything an entry point can do to the world, in one page.
///
/// It is the same walk a deploy does — build the call graph, follow every call through the
/// interface bindings `resolve` recorded, mark what writes storage and what leaves the
/// repository — but printed instead of drawn. A deploy answers "how does this work"; this
/// answers "what does it touch", which is the question you take to a spec.
///
/// Nothing here is new analysis. The value is that the analysis already existed and could
/// only be read by looking at thirty frames on a board.
pub async fn effects(options: AutoDeployOptions) -> Result<()> {
    let metadata = EvmBatMetadata::read_metadata().change_context(EvmMiroError)?;
    let mut options = options;
    for pattern in &metadata.ignored_contracts {
        if !options.ignore_contracts.contains(pattern) {
            options.ignore_contracts.push(pattern.clone());
        }
    }
    let options = options;

    let targets = select_targets(&metadata, &options)?;
    let Some((contract_name, function_name, root_file)) = targets.into_iter().next() else {
        return Err(Report::new(EvmMiroError)
            .attach_printable("no entry point matched; run `bat-cli sonar` first"));
    };

    let title = format!("{contract_name}.{function_name}");
    let (nodes, edges, unresolved) =
        build_graph(&metadata, &contract_name, &function_name, &root_file, &options)?;
    if nodes.is_empty() {
        return Err(Report::new(EvmMiroError)
            .attach_printable(format!("no function metadata for {title}")));
    }

    let depth = nodes.iter().map(|node| node.depth).max().unwrap_or(0) + 1;

    // Built as plain lines and then printed, so the terminal and the board show exactly the
    // same report rather than two renderings that can drift apart.
    let mut report: Vec<String> = Vec::new();
    report.push(format!(
        "{title} — {} function(s) reached, {depth} level(s) deep",
        nodes.len()
    ));

    // A TREE, not a list of routes. Every route shares the same prefix — `swap → execute →
    // …` was on every line — and repeating it is noise that hides the shape. Each function
    // appears once, indented under the one that reaches it, and carries its own effects.
    //
    // Pruned to the branches that lead somewhere: of 166 functions reached, the ones that
    // change nothing and call nothing that changes anything are not what the question is
    // about.
    let (children, order) = call_tree(&nodes, &edges);
    let by_id: HashMap<&str, &GraphNode> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    let writes_count: usize = nodes.iter().map(|n| n.write_lines.len()).sum();
    let boundary_count: usize = nodes.iter().map(|n| n.external_call_lines.len()).sum();

    report.push(String::new());
    for note in [
        "This is the UNION over every branch, not one execution: which of these happen",
        "depends on the path taken, and under WHICH condition a branch is taken is not",
        "something this tool knows. Branches that change nothing are left out.",
    ] {
        report.push(note.to_string());
    }
    // Two trees, not one with two kinds of mark on it. They answer different questions —
    // "what does this change" and "where does value leave" — and a reader following one of
    // them should not have to step over the other. On the board they become two screenshots
    // side by side, so the two answers are compared rather than scrolled between.
    let root = nodes[0].id.clone();
    let mut sections: Vec<Vec<String>> = Vec::new();
    for effect in [Effect::State, Effect::Boundary] {
        let mut section = vec![
            match effect {
                Effect::State => format!("{title} — ● state changes ({writes_count})"),
                Effect::Boundary => format!(
                    "{title} — ▲ external boundaries ({boundary_count}), where value can move"
                ),
            },
            "the union over every branch; which happen depends on the path taken".to_string(),
            String::new(),
        ];
        let keep = branches_that_matter(&nodes, &children, effect);
        if keep.is_empty() {
            section.push(match effect {
                Effect::State => "  nothing here writes contract storage".to_string(),
                Effect::Boundary => "  nothing here leaves the code in scope".to_string(),
            });
        } else {
            write_subtree(&root, "", "", &children, &by_id, &keep, &order, effect, &mut section);
        }
        report.push(String::new());
        report.extend(section.iter().cloned());
        sections.push(section);
    }

    // Said last and said plainly: an unresolved interface is a branch this walk did not
    // follow, so everything above is a lower bound on what the entry point can do.
    if !unresolved.is_empty() {
        report.push(String::new());
        report.push(format!(
            "{} interface call(s) were NOT followed, so the list above is a floor:",
            unresolved.len()
        ));
        for call in &unresolved {
            let kind = if call.inferred_type.is_empty() {
                String::new()
            } else {
                format!(" ({})", call.inferred_type)
            };
            report.push(format!("  {}.{}{}", call.receiver, call.method, kind));
        }
        report.push("  fix: bat-cli resolve <INTERFACE> <CONTRACT>, then run this again".to_string());
    }

    println!();
    for line in &report {
        println!("{line}");
    }

    if options.deploy_effects {
        deploy_effects(&title, &sections).await?;
    }

    Ok(())
}

/// Put the report on the board, as one frame of its own.
///
/// The terminal is where you read it once; the board is where it sits next to the diagram it
/// describes, so "what does this touch" and "how does it work" are the same glance apart.
/// It is placed below everything already there, re-read from the board, so it cannot land on
/// another frame — Miro refuses that outright.
async fn deploy_effects(title: &str, sections: &[Vec<String>]) -> Result<()> {
    use crate::batbelt::path::BatFolder;

    BatFolder::Figures.create_folder().change_context(EvmMiroError)?;
    let destination = BatFolder::Figures.get_path(true).change_context(EvmMiroError)?;

    const MARGIN: f64 = 400.0;
    const GAP: f64 = 400.0;

    let mut rendered: Vec<(String, f64, f64)> = Vec::new();
    for (index, section) in sections.iter().enumerate() {
        let png_path = crate::batbelt::silicon::create_figure(
            &section.join("\n"),
            &destination,
            &format!("effects_{}_{index}.txt", title.replace('.', "_")),
            0,
            Some(REFERENCE_FONT),
            false,
        );
        let (png_width, png_height) =
            image::image_dimensions(&png_path).change_context(EvmMiroError)?;
        rendered.push((png_path, png_width as f64, png_height as f64));
    }

    let width: f64 = rendered.iter().map(|(_, w, _)| w).sum::<f64>()
        + GAP * (rendered.len() as f64 - 1.0);
    let height: f64 = rendered.iter().fold(0.0_f64, |tallest, (_, _, h)| tallest.max(*h));

    let client = MiroClient::new_refreshed().await.change_context(EvmMiroError)?;
    let (frame_x, frame_y) = crate::batbelt::evm::miro::overview::free_spot(
        &client,
        width + MARGIN * 2.0,
        height + MARGIN * 2.0,
    )
    .await?;
    let frame_id = client
        .create_frame(
            &format!("effects: {title}"),
            frame_x,
            frame_y,
            width + MARGIN * 2.0,
            height + MARGIN * 2.0,
            None,
        )
        .await
        .change_context(EvmMiroError)?;

    // Top-aligned, like the overview's columns: the two trees are different lengths and
    // what is being read is each one from its first line.
    let mut cursor = MARGIN;
    for (png_path, png_width, png_height) in &rendered {
        client
            .create_image_in_frame(
                png_path,
                &frame_id,
                title,
                cursor + png_width / 2.0,
                MARGIN + png_height / 2.0,
                *png_width,
            )
            .await
            .change_context(EvmMiroError)?;
        cursor += png_width + GAP;
        let _ = std::fs::remove_file(png_path);
    }
    println!("  {}", client.frame_url(&frame_id).blue());
    Ok(())
}

/// The call graph as a TREE: for each function, the ones it reaches that are not already
/// reached by something shallower, plus the order each was first met in.
///
/// Breadth-first, so a function hangs under the SHORTEST way in and appears exactly once. A
/// function reached by several callers is shown under one of them; the report says so,
/// because pretending otherwise would be the same mistake as listing every branch's effects
/// as though they all happen.
fn call_tree(
    nodes: &[GraphNode],
    edges: &[GraphEdge],
) -> (HashMap<String, Vec<String>>, HashMap<String, usize>) {
    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    let mut order: HashMap<String, usize> = HashMap::new();
    let Some(root) = nodes.first() else {
        return (children, order);
    };

    let mut callees: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in edges {
        callees.entry(edge.from.as_str()).or_default().push(edge.to.as_str());
    }

    let mut seen: HashSet<&str> = HashSet::new();
    seen.insert(root.id.as_str());
    order.insert(root.id.clone(), 0);
    let mut queue: VecDeque<&str> = VecDeque::new();
    queue.push_back(root.id.as_str());
    while let Some(id) = queue.pop_front() {
        for callee in callees.get(id).into_iter().flatten() {
            if !seen.insert(callee) {
                continue;
            }
            order.insert((*callee).to_string(), order.len());
            children.entry(id.to_string()).or_default().push((*callee).to_string());
            queue.push_back(callee);
        }
    }
    (children, order)
}

/// Which question a tree is answering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Effect {
    /// What this changes.
    State,
    /// Where value can leave.
    Boundary,
}

impl Effect {
    fn present_in(self, node: &GraphNode) -> bool {
        match self {
            Self::State => !node.write_lines.is_empty(),
            Self::Boundary => !node.external_call_lines.is_empty(),
        }
    }
}

/// The nodes worth printing for ONE kind of effect: the ones that have it, and every
/// ancestor that leads to one.
///
/// Without this the tree is 166 functions and the six that matter are lost in it.
fn branches_that_matter(
    nodes: &[GraphNode],
    children: &HashMap<String, Vec<String>>,
    effect: Effect,
) -> HashSet<String> {
    let has_effect: HashSet<&str> = nodes
        .iter()
        .filter(|n| effect.present_in(n))
        .map(|n| n.id.as_str())
        .collect();

    let mut keep: HashSet<String> = HashSet::new();
    // Deepest first, so a node is decided after its children are.
    let mut by_depth: Vec<&GraphNode> = nodes.iter().collect();
    by_depth.sort_by(|a, b| b.depth.cmp(&a.depth));
    for node in by_depth {
        let child_matters = children
            .get(&node.id)
            .into_iter()
            .flatten()
            .any(|child| keep.contains(child));
        if child_matters || has_effect.contains(node.id.as_str()) {
            keep.insert(node.id.clone());
        }
    }
    keep
}

/// Print one node and the kept branches under it, indented.
/// Print one node and the kept branches under it, drawn as a tree.
///
/// `prefix` is what precedes this node's own line — the elbow that connects it to its
/// caller — and `continuation` what precedes everything underneath it, so a branch with
/// more siblings keeps a vertical line through its children and the last one does not.
///
/// One effect per line, never a comma-separated run: a function that assigns six fields of
/// the same struct is six things to check, and six things to check do not fit on a line a
/// reader is meant to scan.
#[allow(clippy::too_many_arguments)]
fn write_subtree(
    id: &str,
    prefix: &str,
    continuation: &str,
    children: &HashMap<String, Vec<String>>,
    by_id: &HashMap<&str, &GraphNode>,
    keep: &HashSet<String>,
    order: &HashMap<String, usize>,
    effect: Effect,
    report: &mut Vec<String>,
) {
    let Some(node) = by_id.get(id) else { return };

    let marks: Vec<String> = match effect {
        Effect::State => node
            .write_lines
            .iter()
            .map(|(line, lvalue)| format!("{lvalue}  {}:{line}", prettify(&node.file_path)))
            .collect(),
        Effect::Boundary => node
            .external_call_lines
            .iter()
            .map(|line| format!("{}:{line}", prettify(&node.file_path)))
            .collect(),
    };

    report.push(format!("{prefix}{}", node.label));

    let mut kids: Vec<&String> = children
        .get(id)
        .into_iter()
        .flatten()
        .filter(|child| keep.contains(*child))
        .collect();
    kids.sort_by_key(|child| order.get(*child).copied().unwrap_or(usize::MAX));

    // The effects hang under their own function, before its callees, with the vertical line
    // carried through when there are callees still to come.
    let stem = if kids.is_empty() { "   " } else { "│  " };
    for mark in &marks {
        report.push(format!("{continuation}{stem}{}{mark}", if effect == Effect::State { "● " } else { "▲ " }));
    }

    for (index, child) in kids.iter().enumerate() {
        let last = index + 1 == kids.len();
        let elbow = if last { "└─ " } else { "├─ " };
        let carry = if last { "   " } else { "│  " };
        write_subtree(
            child,
            &format!("{continuation}{elbow}"),
            &format!("{continuation}{carry}"),
            children,
            by_id,
            keep,
            order,
            effect,
            report,
        );
    }
}

/// A path as the screenshots show it — relative to the audited repository.
fn prettify(path: &str) -> String {
    crate::batbelt::path::prettify_source_code_path(path).unwrap_or_else(|_| path.to_string())
}

pub async fn run(options: AutoDeployOptions) -> Result<()> {
    let metadata = EvmBatMetadata::read_metadata().change_context(EvmMiroError)?;

    let targets = select_targets(&metadata, &options)?;
    if targets.is_empty() {
        return Err(Report::new(EvmMiroError)
            .attach_printable("no entry point matched; run `bat-cli sonar` first"));
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

    // A fresh deploy draws into a CLEAN zone: forget the cached region so the allocator
    // re-scans and places the fresh cluster below everything currently on the board.
    if !options.dry_run {
        EvmBatMetadata::update_metadata(|m| m.miro.auto.region = None)
            .change_context(EvmMiroError)?;
    }

    // The board is scanned at most once, to pick the region origin.
    let metadata = EvmBatMetadata::read_metadata().change_context(EvmMiroError)?;
    // What a deploy leaves out is the union of the saved list and this run's flags.
    let mut options = options;
    for pattern in &metadata.ignored_contracts {
        if !options.ignore_contracts.contains(pattern) {
            options.ignore_contracts.push(pattern.clone());
        }
    }
    let options = options;
    if !options.ignore_contracts.is_empty() {
        println!(
            "  {} not drawing: {}",
            "note:".yellow(),
            options.ignore_contracts.join(", ")
        );
    }
    let mut allocator = if options.dry_run {
        ShelfAllocator::new(0.0, 0.0)
    } else {
        resolve_allocator(client.as_ref().unwrap(), &metadata).await?
    };

    for (contract_name, function_name, root_file) in targets {
        let title = format!("{contract_name}.{function_name}");

        // What the PREVIOUS deployment of this entry point was enriched with: every
        // declaration and type drawn onto one of its frames with `bat-cli screenshot`.
        // A deploy draws fresh frames, so those would be left behind on the old ones —
        // and they are the auditor's own reading notes, asked for one at a time. They are
        // put back at the end, on the frame of the same name in the new cluster.
        let previous_extras: Vec<(String, String, bool)> = {
            let meta = EvmBatMetadata::read_metadata().change_context(EvmMiroError)?;
            meta.miro
                .auto
                .frames
                .iter()
                .filter(|f| f.cluster_root == title && !f.type_frame)
                .flat_map(|f| {
                    f.screenshots.iter().map(|shot| {
                        (f.entry_point.clone(), shot.label.clone(), shot.with_documentation)
                    })
                })
                .collect()
        };
        let stale_ids: HashSet<String> = {
            let meta = EvmBatMetadata::read_metadata().change_context(EvmMiroError)?;
            meta.miro
                .auto
                .frames
                .iter()
                // A deployment is its cluster, nothing else: a frame named after this
                // function inside SOMEBODY ELSE's deployment belongs to them.
                .filter(|f| f.cluster_root == title)
                .map(|f| f.frame_id.clone())
                .collect()
        };
        // Deploying an entry point that already has a deployment REPLACES it: the new
        // cluster is drawn fresh and the old frames are left on the board for you to
        // delete. That is a big enough thing to happen by surprise that it asks first.
        let previous: Vec<String> = {
            let meta = EvmBatMetadata::read_metadata().change_context(EvmMiroError)?;
            meta.miro
                .auto
                .frames
                .iter()
                .filter(|f| f.cluster_root == title)
                .map(|f| f.entry_point.clone())
                .collect()
        };
        if !previous.is_empty() && !options.dry_run && !options.assume_yes {
            println!(
                "  {} {} already has a deployment of {} frame(s). Deploying again draws a NEW\n  cluster and leaves the old frames on the board for you to delete.",
                "note:".yellow(),
                title.bold(),
                previous.len()
            );
            if !BatDialoguer::select_yes_or_no("Deploy it again?".to_string())
                .change_context(EvmMiroError)?
            {
                continue;
            }
        }

        let root_options = options.clone();

        // PLAN the whole deployment first: every frame, in reading order, with its size
        // and its slot already decided and nothing on the board touched. One walk decides
        // everything; the drawing below decides nothing (§21).
        let plans = plan_cluster(
            &metadata,
            &contract_name,
            &function_name,
            &root_file,
            &root_options,
            &mut allocator,
        )?;
        println!(
            "\n{} {} frame(s) planned",
            "▦".blue(),
            plans.len().to_string().green()
        );

        if options.dry_run {
            for plan in &plans {
                println!(
                    "\n{} {}{}",
                    "▸".blue(),
                    "  ".repeat(plan.cluster_depth),
                    plan.title.bold()
                );
                print_dry_run(
                    &plan.nodes,
                    &plan.edges,
                    &plan.anchors,
                    &plan.layout,
                    (plan.frame_x, plan.frame_y),
                );
            }
            for plan in &plans {
                cleanup(&plan.nodes);
            }
            continue;
        }

        let client = client.as_ref().expect("client is present when not in dry-run mode");

        // PASS 1: create every frame, empty. After it every card in the deployment knows
        // its destination — which is what lets a frame be filled without waiting for the
        // frames it points at. The plan already fixed where each one goes, so these are
        // independent of each other and go up together; the ids are put back in plan
        // order afterwards, because pass 2 reads them by position.
        let bar = phase_bar("creating frames", plans.len());
        let mut creating = tokio::task::JoinSet::new();
        for (index, plan) in plans.iter().enumerate() {
            let client = client.clone();
            let title = format!("auto: {}", plan.title);
            let fill = frame_fill_for(&plan.nodes);
            let (x, y) = (plan.frame_x, plan.frame_y);
            let (width, height) = (plan.layout.frame_width, plan.layout.frame_height);
            let bar = bar.clone();
            creating.spawn(async move {
                let frame_id = client
                    .create_frame(&title, x, y, width, height, fill)
                    .await
                    .map_err(|report| report.change_context(EvmMiroError))?;
                bar.inc(1);
                Ok::<(usize, String), Report<EvmMiroError>>((index, frame_id))
            });
        }
        let mut created: Vec<Option<String>> = vec![None; plans.len()];
        while let Some(joined) = creating.join_next().await {
            let (index, frame_id) = joined
                .map_err(|e| Report::new(EvmMiroError).attach_printable(e.to_string()))??;
            created[index] = Some(frame_id);
        }
        bar.finish_and_clear();
        // Every slot is filled or the loop above returned the error, so a gap here would
        // be a bug in this function rather than something the board did.
        let frame_ids: Vec<String> = created
            .into_iter()
            .map(|id| id.expect("every planned frame was created or the run failed"))
            .collect();
        let urls: HashMap<String, String> = plans
            .iter()
            .zip(frame_ids.iter())
            .map(|(plan, frame_id)| (plan.title.clone(), client.frame_url(frame_id)))
            .collect();

        // PASS 2: fill them. Every frame's contents are independent of every other's —
        // the plan fixed the geometry and pass 1 fixed the destinations — so they go up
        // several at a time. The ceiling that matters is the client's own semaphore and
        // Miro's credit budget, not this number: a single frame uploads in short bursts
        // (a dozen to forty calls) and leaves the connection idle while the next one's
        // records are written, which is exactly the gap this fills.
        let urls = std::sync::Arc::new(urls);
        let plans: Vec<std::sync::Arc<FramePlan>> =
            plans.into_iter().map(std::sync::Arc::new).collect();
        let mut pending = plans.iter().cloned().zip(frame_ids.into_iter());
        let mut drawing = tokio::task::JoinSet::new();
        let mut failure: Option<Report<EvmMiroError>> = None;
        loop {
            while drawing.len() < CONCURRENT_FRAMES {
                let Some((plan, frame_id)) = pending.next() else {
                    break;
                };
                let urls = urls.clone();
                let options = root_options.clone();
                let client = client.clone();
                let cluster_root = title.clone();
                drawing.spawn(async move {
                    draw_one(&plan, &frame_id, &cluster_root, &urls, &options, &client).await
                });
            }
            let Some(joined) = drawing.join_next().await else {
                break;
            };
            // One frame failing must not cost the thirty that worked: the rest finish and
            // the failure is reported once, at the end.
            match joined {
                Ok(Ok(())) => {}
                Ok(Err(report)) => failure = failure.or(Some(report)),
                Err(join_error) => {
                    failure = failure.or_else(|| {
                        Some(Report::new(EvmMiroError).attach_printable(join_error.to_string()))
                    })
                }
            }
        }
        if let Some(report) = failure {
            return Err(report);
        }

        // The new deployment REPLACES the previous deployment of this entry point: its
        // records go, whole, identified by the frame ids they carried before this run
        // started. The frames themselves stay on the board — the API deletes one item at
        // a time and slowly — so their URLs are printed for one-click deletion by hand.
        if !options.dry_run {
            let old_ids: HashSet<String> = stale_ids.clone();
            EvmBatMetadata::update_metadata(move |m| {
                m.miro.auto.frames.retain(|f| !old_ids.contains(&f.frame_id));
            })
            .change_context(EvmMiroError)?;
        }

        // Put the auditor's drawings back, now that the new cluster is recorded and the
        // old one forgotten, so `--deployment/--dependency` resolves to the new frames. A
        // symbol that no longer exists (the code moved on) is reported and skipped: this
        // is a redraw of notes, not a reason to fail a deploy that already worked.
        if !options.dry_run && !previous_extras.is_empty() {
            println!(
                "\n  {} putting back {} drawing(s) from the previous deployment",
                "↻".yellow(),
                previous_extras.len()
            );
            for (frame, symbol, with_documentation) in previous_extras {
                let dependency = (frame != title).then(|| frame.clone());
                let outcome = crate::batbelt::evm::miro::screenshot::run(
                    crate::batbelt::evm::miro::screenshot::ScreenshotOptions {
                        name: Some(symbol.clone()),
                        deployment: Some(title.clone()),
                        dependency,
                        file: None,
                        lines: None,
                        with_documentation,
                        grow: false,
                    },
                )
                .await;
                if let Err(report) = outcome {
                    println!(
                        "    {} {} on {}: {}",
                        "skipped".yellow(),
                        symbol,
                        frame,
                        report.current_context()
                    );
                }
            }
        }

        // Persist the cursor after every entry point, not once at the end: a run
        // over a whole project is long enough to be interrupted, and a lost
        // cursor would place the next batch on top of the frames already there.
        if !options.dry_run {
            let state = ShelfState::from(&allocator);
            EvmBatMetadata::update_metadata(|m| m.miro.auto.region = Some(state.clone()))
                .change_context(EvmMiroError)?;
        }
    }

    // Wipe the shared screenshot cache once, now that every frame (a whole cluster)
    // has uploaded — each distinct function was rendered once for the entire run.
    if !options.dry_run {
        if let Ok(dir) = BatFolder::Figures.get_path(false) {
            let _ = std::fs::remove_dir_all(&dir);
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
) -> Result<Vec<(String, String, String)>> {
    // A target is `(contract, function, file)`. The file is part of the identity: `lib/`
    // can vendor three copies of `BeaconProxy`, all with the same name.
    let entry_names: HashSet<(String, String)> = metadata
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

    // The picker and `--all` list what the project itself defines: `lib/` and
    // constructors are dependency plumbing nobody wants to scroll past. Naming something
    // explicitly with `--entry-point` reaches everything — see `resolve_named_target`.
    let mut entry_points: Vec<(String, String, String)> = Vec::new();
    let mut others: Vec<(String, String, String)> = Vec::new();
    for contract in metadata.contracts.iter().filter(|c| !c.external) {
        for function in &contract.functions {
            let target = (
                contract.name.clone(),
                function.name.clone(),
                contract.file_path.clone(),
            );
            if entry_names.contains(&(contract.name.clone(), function.name.clone())) {
                entry_points.push(target);
            } else if !function.is_constructor {
                others.push(target);
            }
        }
    }
    entry_points.sort();
    entry_points.dedup();
    others.sort();
    others.dedup();

    if let Some(wanted) = &options.entry_point {
        return resolve_named_target(metadata, &entry_names, wanted);
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

    let all: Vec<(String, String, String)> = entry_points
        .iter()
        .cloned()
        .chain(others.iter().cloned())
        .collect();
    if all.is_empty() {
        return Ok(all);
    }

    // Two in-scope contracts can share a name too; only then is the path worth showing.
    let mut title_count: HashMap<String, usize> = HashMap::new();
    for (contract, function, _) in &all {
        *title_count.entry(format!("{contract}.{function}")).or_default() += 1;
    }

    let entry_point_count = entry_points.len();
    let labels: Vec<String> = all
        .iter()
        .enumerate()
        .map(|(index, (contract, function, file))| {
            let title = format!("{contract}.{function}");
            let mut label = if title_count[&title] > 1 {
                format!("{title}  ({file})")
            } else {
                title.clone()
            };
            if index < entry_point_count {
                label = format!("{label}  {}", "[entry point]".blue());
            }
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

/// Resolve `--entry-point` to exactly one function, or stop and say why.
///
/// Accepts `function`, `Contract.function`, and `path/To.sol:Contract.function`. Named
/// explicitly, anything is reachable: a contract under `lib/`, a constructor, a
/// `fallback`. Following what a constructor does is a legitimate thing to draw, and the
/// person who typed the name has already decided it is worth seeing.
///
/// Several matches are narrowed in the order a reader would expect, and only what the code
/// cannot decide becomes a question:
///
/// 1. the project's own entry points, then its other functions, then `lib/` — so `mint`
///    still means the audited `DebtToken.mint`, not one of OpenZeppelin's;
/// 2. among same-named contracts in different files, the copy the audited code imports,
///    resolved through the import graph and `remappings.txt` exactly as `solc` resolves it;
/// 3. anything still ambiguous — two in-scope contracts with a `poke`, or a library nobody
///    in `src/` imports — stops, listing each candidate in the `path:Contract.function`
///    form that selects it. That decision needs context the code does not carry, which is
///    exactly when the assistant (or the auditor) should make it.
fn resolve_named_target(
    metadata: &EvmBatMetadata,
    entry_names: &HashSet<(String, String)>,
    wanted: &str,
) -> Result<Vec<(String, String, String)>> {
    use crate::batbelt::evm::parser::import_graph::{normalize, reachable_from};

    let (wanted_path, rest) = match wanted.rsplit_once(':') {
        Some((path, rest)) if path.ends_with(".sol") => (Some(normalize(path)), rest),
        _ => (None, wanted),
    };
    let (wanted_contract, wanted_function) = match rest.rsplit_once('.') {
        Some((contract, function)) => (Some(contract), function),
        None => (None, rest),
    };

    // (contract, function, file, external, is entry point)
    let mut matches: Vec<(String, String, String, bool, bool)> = Vec::new();
    for contract in &metadata.contracts {
        // An interface declares, it does not do anything worth drawing.
        if contract.contract_type == EvmContractType::Interface {
            continue;
        }
        if wanted_contract.is_some_and(|name| name != contract.name) {
            continue;
        }
        if let Some(path) = &wanted_path {
            let file = normalize(&contract.file_path);
            if file != *path && !file.ends_with(&format!("/{path}")) {
                continue;
            }
        }
        for function in contract.functions.iter().filter(|f| f.name == wanted_function) {
            let entry = !contract.external
                && entry_names.contains(&(contract.name.clone(), function.name.clone()));
            matches.push((
                contract.name.clone(),
                function.name.clone(),
                contract.file_path.clone(),
                contract.external,
                entry,
            ));
        }
    }
    // Overloads of one function in one contract are one target; the graph picks the overload.
    matches.sort();
    matches.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1 && a.2 == b.2);
    if matches.is_empty() {
        return Ok(Vec::new());
    }

    let tier: Vec<&(String, String, String, bool, bool)> = {
        let entry: Vec<_> = matches.iter().filter(|m| m.4).collect();
        let own: Vec<_> = matches.iter().filter(|m| !m.3).collect();
        if !entry.is_empty() {
            entry
        } else if !own.is_empty() {
            own
        } else {
            matches.iter().collect()
        }
    };
    let pick = |m: &(String, String, String, bool, bool)| vec![(m.0.clone(), m.1.clone(), m.2.clone())];
    if tier.len() == 1 {
        return Ok(pick(tier[0]));
    }

    // Same name, several files: keep the copies the audited code can actually reach.
    let reachable = reachable_from(
        metadata
            .contracts
            .iter()
            .filter(|c| !c.external)
            .map(|c| c.file_path.as_str()),
    );
    let narrowed: Vec<_> = tier
        .iter()
        .filter(|m| reachable.contains(&normalize(&m.2)))
        .copied()
        .collect();
    if narrowed.len() == 1 {
        return Ok(pick(narrowed[0]));
    }

    let listed = if narrowed.is_empty() { &tier } else { &narrowed };
    let candidates = listed
        .iter()
        .map(|m| format!("{}:{}.{}", normalize(&m.2), m.0, m.1))
        .collect::<Vec<_>>();
    let why = if narrowed.is_empty() {
        "nothing in the audited code imports any of them, so the code cannot decide"
    } else {
        "the audited code reaches more than one of them"
    };
    Err(Report::new(EvmMiroError)
        .attach_printable(format!(
            "`{wanted}` matches {} functions — {why}:\n    {}",
            listed.len(),
            candidates.join("\n    ")
        ))
        .attach(crate::Suggestion(format!(
            "pick one and pass it whole, e.g. `bat-cli deploy --entry-point {}`",
            candidates[0]
        ))))
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

/// Where the k-th way-back card goes: the bottom-right corner of the frame, stacking
/// upward, or `None` when the content reaches down there.
///
/// Bottom-right because that is where a frame has room — the entry point sits top-left and
/// the graph fans down and to the right, so the corner under the last column is usually
/// empty. Inside the frame, not beside it, so dragging the frame takes the way back along.
fn back_card_slot(
    frame_width: f64,
    frame_height: f64,
    occupied: &[(f64, f64, f64, f64)],
    index: usize,
) -> Option<(f64, f64)> {
    const MARGIN: f64 = 200.0;
    let x = frame_width - MARGIN - LINK_CARD_WIDTH / 2.0;
    let y = frame_height
        - MARGIN
        - LINK_CARD_HEIGHT / 2.0
        - index as f64 * (LINK_CARD_HEIGHT + MARGIN / 2.0);
    if y - LINK_CARD_HEIGHT / 2.0 < MARGIN {
        return None; // stacked past the top of the frame
    }
    let clear = occupied.iter().all(|(ox, oy, ow, oh)| {
        (x - ox).abs() * 2.0 >= LINK_CARD_WIDTH + ow
            || (y - oy).abs() * 2.0 >= LINK_CARD_HEIGHT + oh
    });
    clear.then_some((x, y))
}


/// Plan ONE frame: everything local. No client, no await, nothing on the board — it
/// builds the graph, frames it, lays it out, renders its screenshots and takes its slot.
/// `plan_cluster` is what walks a whole deployment with it.
fn plan_one(
    metadata: &EvmBatMetadata,
    contract_name: &str,
    function_name: &str,
    // The file of the selected contract, which is what tells same-named copies apart.
    root_file: &str,
    options: &AutoDeployOptions,
    allocator: &mut ShelfAllocator,
    // How deep this frame sits in the CLUSTER: the root is 0, what its cards lead to is
    // 1, and so on. It is the frame's indent on the board — see `place_in_outline`. Named
    // `cluster_depth`, not `depth`, because this function already has a `graph_depth`
    // (how many levels of calls the frame draws inside itself) and shadowing the two cost
    // a whole deployment.
    cluster_depth: usize,
    // The functions this deployment has ALREADY given a frame to. Cutting to one of them
    // costs no new frame, so `best_cut` may target it at any size. It used to be read back
    // from the registry mid-deploy; the planner knows it directly, and a single source for
    // it is what keeps the plan and the board from disagreeing (§21).
    framed: &HashSet<String>,
) -> Result<Option<FramePlan>> {
    let title = format!("{contract_name}.{function_name}");
    println!("\n{} {}", "▸".blue(), title.bold());
    // When several contracts share this name, say which copy was picked. The choice is made
    // from the imports and is deterministic, but nobody reading the output should have to
    // go re-derive it from `remappings.txt` to trust the diagram.
    if metadata.contracts.iter().filter(|c| c.name == contract_name).count() > 1 {
        println!("  {} {}", "from".dimmed(), root_file);
    }

    // One frame per function, board-wide. Asked for as a link target, a function
    // already on the board is pointed at rather than drawn again — that is what
    // lets several diagrams share a helper's frame and keeps the fan-in readable.
    // Asked for directly, RECYCLE it: delete the old frame and its items, then
    // redraw — so a redeploy replaces the frame instead of piling up duplicates.
    let (mut nodes, mut edges, unresolved) =
        build_graph(metadata, contract_name, function_name, root_file, options)?;
    if nodes.is_empty() {
        println!("  no function metadata found, skipping");
        return Ok(None);
    }

    // Every deploy is fresh, so nothing another deploy left on the board is linked —
    // but a frame THIS run already drew for THIS cluster is a different matter. It is
    // the same rule `ensure_target_frames` reuses by (`cluster_root` matches, id is not
    // the old cluster's), and it is what keeps a helper two branches reach from being
    // drawn twice: the second branch cards it. Cutting to one of these is free — the
    // frame exists — so `best_cut` may target it at any size, which is what you want
    // for a big callee that keeps coming back.
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
    // How many levels of calls this frame draws INSIDE itself — not `cluster_depth`.
    let graph_depth = nodes.iter().map(|node| node.depth).max().unwrap_or(0);
    println!(
        "  {} screenshots, {} connectors, {} levels deep",
        nodes.len().to_string().green(),
        edges.len().to_string().green(),
        (graph_depth + 1).to_string().green()
    );

    let reuse: HashMap<String, (String, u32, u32)> = HashMap::new();

    render_and_measure(&mut nodes, &title, &reuse)?;

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
    // Functions with a frame already on the board: cutting to one is free (no new
    // frame is created), so `best_cut` may target them at any size.
    let framed: HashSet<&str> = framed.iter().map(|s| s.as_str()).collect();
    // Aim each piece AT a readable size: the budget starts at the target, raised only
    // when the graph is so big that even `MAX_CUTS_PER_FRAME` target-sized cuts wouldn't
    // fit it — then the pieces are bigger and split again by their own deploy.
    let total = screenshot_count(&nodes);
    let mut budget = FRAME_TARGET.max(total.div_ceil(MAX_CUTS_PER_FRAME + 1));
    let mut anchors = anchors;
    // Cut until the frame READS, not a fixed number of times. The cap was a fixed ten
    // passes and a budget that never moved, so a graph whose branches are all smaller
    // than the budget — or all shared — exhausted its passes (or found nothing to cut
    // at all) and shipped whatever was left: `FLAMMSwapLib.execute` landed on the board
    // as 250 screenshots and 641 connectors. The bound is now the node count, which no
    // run can reach: every pass removes at least one screenshot.
    let cut_passes = if options.inline_all { 0 } else { nodes.len() };
    for _ in 0..cut_passes {
        if effective_size(&nodes) <= FRAME_MAX {
            break;
        }
        // Nothing worth cutting AT THIS BUDGET is not the same as nothing worth
        // cutting: aiming at readable pieces first, then smaller ones, beats giving up
        // and drawing a wall. FRAME_MIN is the floor — below it a cut buys a husk frame
        // and costs a card, which is worse than the screenshot it replaced.
        let mut found = None;
        while found.is_none() {
            found = best_cut(&nodes, &edges, &framed, budget);
            if found.is_some() || budget <= FRAME_MIN {
                break;
            }
            budget = (budget * 3 / 4).max(FRAME_MIN);
        }
        let Some((cut_nodes, cut_edges)) = found else {
            println!(
                "  {} {} screenshots and nothing left that can be cut into a readable\n  frame — every branch is either shared or too small to be worth its own",
                "warning:".yellow(),
                screenshot_count(&nodes)
            );
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

    // Localize the small helpers that STILL cross after framing. Now that the big
    // shared subtrees are cut out to their own frames, the frame is small, so
    // copying a leaf (sqrt, a getter) next to each far caller is cheap and — crucial
    // — no longer whack-a-mole (the deep floor those copies would re-call is gone).
    // Uses the measured layout (a caller ≥2 columns back = a crossing arrow), copies
    // only SMALL closures, then re-lays-out.
    //
    // That first sentence is a PREMISE, not a description, so it is checked: on a frame
    // framing could not bring under FRAME_MAX, copying helpers adds screenshots to a
    // wall to shorten arrows nobody can follow anyway. Shortening an unreadable diagram
    // is not worth making it bigger, so localization is skipped there.
    if effective_size(&nodes) <= FRAME_MAX {
        // First the crossers too big to copy: they become frames of their own, with a
        // card beside each caller, so the long arrow is gone rather than shortened.
        let cut_crossers = if options.inline_all {
            0
        } else {
            cut_crossing_shared(&mut nodes, &mut edges, &root_id)
        };
        if cut_crossers > 0 {
            println!(
                "  {} {} call(s) flying over a column replaced by a card to the callee's frame",
                "↳".blue(),
                cut_crossers
            );
        }
        let before = screenshot_count(&nodes);
        duplicate_crossing_shared(&mut nodes, &mut edges, &root_id);
        // And again for whatever still crosses. Copying is what makes a later victim look
        // bigger: every copy takes a helper out of the shared set, so the private closure
        // of the callees still waiting GROWS as the pass runs — one real callee went from
        // 3 to 6 while others were copied, crossed the "small enough to copy" line before
        // its turn came, and kept its arrow flying over a column because the pass that
        // would have carded it had already finished. The two bands meet at FRAME_MIN, so
        // the cut has to be offered the second half of that walk too.
        let late_cuts = if options.inline_all {
            0
        } else {
            cut_crossing_shared(&mut nodes, &mut edges, &root_id)
        };
        if late_cuts > 0 {
            println!(
                "  {} {} more call(s) flying over a column replaced by a card",
                "↳".blue(),
                late_cuts
            );
        }
        if screenshot_count(&nodes) > before {
            println!(
                "  {} localized {} crossing helper copy(ies)",
                "↳".blue(),
                screenshot_count(&nodes) - before
            );
        }
        anchors = compute_anchors(&nodes, &edges);
        let ln: Vec<LayoutNode> = nodes
            .iter()
            .map(|node| LayoutNode {
                id: node.id.clone(),
                width: node.board_width(),
                height: node.board_height(),
            })
            .collect();
        let le: Vec<LayoutEdge> = edges
            .iter()
            .zip(anchors.iter())
            .map(|(edge, anchor)| LayoutEdge {
                from: edge.from.clone(),
                to: edge.to.clone(),
                from_line_fraction: anchor.y_fraction,
            })
            .collect();
        layout = layout_graph(&root_id, &ln, &le, LayoutConfig::default());
    }
    // The amber lines a node shows: the boundaries in its own body, plus the calls that
    // reach one the frame does not draw.
    let drawn_for_amber = drawn_screen_ids(&nodes);
    let amber_lines: HashMap<String, Vec<usize>> = nodes
        .iter()
        .map(|node| {
            let mut lines = node.external_call_lines.clone();
            lines.extend(surviving_external_calls(node, &drawn_for_amber).map(|(line, _, _)| *line));
            lines.sort_unstable();
            lines.dedup();
            (node.id.clone(), lines)
        })
        .collect();
    for node in nodes.iter_mut() {
        if let Some(lines) = amber_lines.get(&node.id) {
            node.external_call_lines = lines.clone();
        }
    }

    // The cards this frame will hand out, in the order a reader meets them. Computed
    // HERE, before the frame takes its slot, because whether this frame has a subtree
    // underneath decides whether it can share its row — and the slot is taken first.
    let card_order = card_reading_order(&nodes, &edges, &root_id);

    // Reserve the slot in both modes, so a dry run shows the real sequence of
    // board positions instead of repeating the first one.
    // Every frame is brand new, so it always takes a slot from the allocator — which
    // is what puts a cluster's frames next to each other instead of wherever an
    // earlier deploy happened to leave them. The depth is the indent: the cluster is
    // deployed in reading order, so laying it out as an outline makes scrolling down
    // the board the same thing as reading the cluster.
    let (frame_x, frame_y) = allocator.place_in_outline(
        layout.frame_width,
        layout.frame_height,
        cluster_depth,
        !card_order.is_empty(),
    );

    if let Some(preview_path) = &options.preview {
        let path = preview_path.clone();
        render_preview(&nodes, &edges, &anchors, &layout, &path)?;
        println!("  preview written to {}", path.blue());
        // A preview is a LOCAL composition for eyeballing the layout — it never
        // touches the board. (The delete+recreate of a redeploy is slow, so this is
        // the fast way to iterate on the diagram.) Stop here, like a dry run.
        cleanup(&nodes);
        return Ok(None);
    }

    let plan = FramePlan {
        title,
        cluster_depth,
        nodes,
        edges,
        anchors,
        layout,
        frame_x,
        frame_y,
        card_order,
        origins: Vec::new(),
    };

    Ok(Some(plan))
}

/// How many frames are filled at once. Each one's uploads are already concurrent inside
/// the client (`MAX_CONCURRENT_REQUESTS` = 24), so this does not raise the ceiling — it
/// keeps that pipe full while a frame is in a phase that serialises, which the connectors
/// are: a connector group is a chain (marker → connector → lane) and one frame drawing
/// them leaves the connection half idle.
///
/// Measured on `FLAMM.swap` (30 frames, ~5 700 writes): one at a time took 776 s to draw;
/// four took 353 s with zero retries and zero waits on the credit budget. The budget is
/// 100 000 credits/minute at 100 per write — 1 000 writes/minute — which puts a floor of
/// about 5.7 minutes on this cluster whatever this number is, so there was room to raise
/// it. Going past the client's 24 permits cannot help.
const CONCURRENT_FRAMES: usize = 16;

/// The names worth following through a screenshot: the function's parameters first, then
/// its local variables, and only as many as the palette can tell apart.
///
/// Parameters come first because they are what differs between call sites — the reason to
/// read this function rather than another. Locals fill the remaining slots by how often
/// they are used, busiest first, which is the same rule `color_callees` applies to arrows:
/// spend the palette where a reader is most likely to lose the thread, and leave the rest
/// unmarked rather than reuse a colour.
///
/// Locals come from the AST (`extract_local_types`), not from a pattern over the text. A
/// tuple declaration, a `for` initialiser and a field access that merely looks like one are
/// exactly the cases a pattern gets wrong, and a wrong mark is worse than no mark.
fn traced_names(lines: &[String]) -> Vec<TracedName> {
    use crate::batbelt::silicon::{TraceKind, TRACE_COLORS};
    let limit = TRACE_COLORS.len();

    // NOT capped. Past the palette the colours start over — two names of a kind can share a
    // hue, told apart by whether their rule is solid or broken, and past that by nothing at
    // all. A repeated colour on two variables is something a reader can work out from
    // context; a variable with no mark cannot be followed at all, which is worse.
    let parameters = signature_parameters(lines);

    let body = lines.join("\n");
    let mut carried: Vec<String> = named_returns(lines)
        .into_iter()
        .filter(|name| !parameters.contains(name))
        .collect();

    // A loop counter is a local the compiler sees and a reader does not need: its whole
    // life is the three tokens of the `for` header. Leaving it out is also what frees a
    // colour for a name that is genuinely hard to follow.
    let counters = crate::batbelt::evm::parser::call_resolver::extract_loop_variables(&body);
    let mut locals: Vec<String> =
        crate::batbelt::evm::parser::call_resolver::extract_local_types(&body)
            .into_iter()
            .map(|(name, _)| name)
            .chain(yul_locals(&body))
            .filter(|name| {
                name != "$"
                    && !counters.contains(name)
                    && !parameters.contains(name)
                    && !carried.contains(name)
            })
            .collect();
    locals.sort();
    locals.dedup();

    let uses = |name: &str| crate::batbelt::silicon::count_word(&body, name);
    locals.sort_by(|a, b| uses(b).cmp(&uses(a)).then(a.cmp(b)));

    // The named return shares the underlined sequence with the locals rather than starting
    // its own: it IS a local by nature, and sharing the counter is what stops it taking a
    // colour one of them already has.
    let returns = carried.len();
    carried.extend(locals);

    // Parameters draw from their own palette, because their background keeps them apart
    // whatever hue they get.
    let parameter_count = parameters.len();
    let palette = crate::batbelt::silicon::TRACE_COLORS.len();
    let mut traced: Vec<TracedName> = parameters
        .into_iter()
        .enumerate()
        .map(|(index, name)| TracedName {
            name,
            kind: TraceKind::Parameter,
            color: index % palette,
            // Solid for the first pass through the palette, broken for the second, and
            // round again: the mark repeats rather than running out.
            dotted: (index / palette) % 2 == 1,
        })
        .collect();
    // The underlined sequence starts PAST the parameters instead of at zero. Both kinds
    // would otherwise open on the same hue, and `lnWad(int256 x) returns (int256 r)` put a
    // salmon `x` and a salmon `r` on every line — on names one character wide the rule
    // underneath is too small to separate them.
    let underlined = crate::batbelt::silicon::UNDERLINED_TRACE_COLORS.len();
    traced.extend(carried.into_iter().enumerate().map(|(index, name)| TracedName {
        name,
        kind: if index < returns { TraceKind::NamedReturn } else { TraceKind::Local },
        color: (parameter_count + index) % underlined,
        dotted: (index / underlined) % 2 == 1,
    }));
    traced
}

/// The variables an inline assembly block declares: `let p := sub(…)`, and the several at
/// once of `let a, b := f()`.
///
/// Yul is not Solidity, so it is not in the statement tree `extract_local_types` walks —
/// in `FixedPointMathLib.lnWad` the two busiest names in the function, `p` and `q`, are
/// declared there and were the only ones left unmarked.
fn yul_locals(body: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in body.lines() {
        let line = line.split("//").next().unwrap_or(line);
        let Some(at) = line.find("let ") else { continue };
        let before_ok = line[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '$'));
        if !before_ok {
            continue;
        }
        let rest = &line[at + 4..];
        let declared = rest.split(":=").next().unwrap_or(rest);
        for part in declared.split(',') {
            let name = part.trim();
            if !name.is_empty()
                && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$')
                && !name.chars().next().is_some_and(|c| c.is_ascii_digit())
            {
                names.push(name.to_string());
            }
        }
    }
    names
}

/// The names a function's `returns (…)` clause declares, when it names them.
///
/// `returns (Plan memory p)` makes `p` the value the whole function is building, and in a
/// body like `_plan`'s it is on nearly every line — exactly the thread a reader loses. It
/// is marked like a local, because that is what it is, but never in a colour a local
/// already took.
fn named_returns(lines: &[String]) -> Vec<String> {
    let joined = lines.join("\n");
    let Some(at) = joined.find("returns") else {
        return Vec::new();
    };
    let Some(open) = joined[at..].find('(').map(|p| at + p) else {
        return Vec::new();
    };
    let Some(close) = matching_paren(&joined, open) else {
        return Vec::new();
    };
    split_parameters(&joined[open + 1..close])
}

/// The parameters a function declares, in order, taken from its own signature.
///
/// They are what a reader most needs to follow through a body and what an editor gives for
/// free on a click; on a PNG there is no click, so each one is drawn in its own colour
/// (`silicon::TRACE_COLORS`) and the signature doubles as the legend. Read from the slice
/// rather than from the AST because the slice is what gets rendered — a screenshot that
/// starts mid-function, or carries its NatSpec, still colours exactly what it shows.
///
/// `lines` is the rendered slice, header included. The result is capped at the palette:
/// past six colours a reader stops being able to tell them apart, so the rest stay plain.
fn signature_parameters(lines: &[String]) -> Vec<String> {
    let joined = lines.join("\n");
    let Some(open) = joined.find("function ").and_then(|at| joined[at..].find('(').map(|p| at + p))
    else {
        return Vec::new();
    };
    let Some(close) = matching_paren(&joined, open) else {
        return Vec::new();
    };
    // Not capped here: how many names can be MARKED is a question about the palette and
    // the decorations, which `traced_names` owns. This just reads the signature.
    split_parameters(&joined[open + 1..close])
}

/// The index of the `)` that closes the `(` at `open`, counting nesting.
fn matching_paren(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, character) in text[open..].char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + index);
                }
            }
            _ => {}
        }
    }
    None
}

/// The declared names in a comma-separated parameter list, ignoring commas nested inside a
/// type (`mapping(uint => uint)`, `uint256[2]`).
fn split_parameters(list: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    for character in list.chars() {
        match character {
            '(' | '[' => {
                depth += 1;
                current.push(character);
            }
            ')' | ']' => {
                depth = depth.saturating_sub(1);
                current.push(character);
            }
            ',' if depth == 0 => {
                names.extend(parameter_name(&current));
                current.clear();
            }
            _ => current.push(character),
        }
    }
    names.extend(parameter_name(&current));
    names
}

/// The declared name in one parameter — the LAST word of `FLAMMStore.S storage $` or of
/// `uint256 amountIn`. A parameter with no name at all (legal in Solidity, and common in
/// an override that ignores one) contributes nothing to follow.
fn parameter_name(declaration: &str) -> Option<String> {
    let words: Vec<&str> = declaration.split_whitespace().collect();
    // A declaration of ONE word is a type with no name: `bytes32`, `address`, and equally
    // `Plan` — a name can only follow a type, so there is nothing to follow here. Reading
    // the last word alone made `returns (bytes32)` look like a variable called `bytes32`,
    // and would have done the same to any user-defined type.
    if words.len() < 2 {
        return None;
    }
    let word = words.last()?;
    let word = word.trim_matches(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'));
    // What is left after the length rule: a data location or modifier standing where a
    // name would be, as in `address payable` or `uint256[] calldata`.
    let is_type = |w: &str| {
        matches!(w, "memory" | "calldata" | "storage" | "payable" | "indexed")
            || w.contains('.')
    };
    // `$` is the storage pointer by convention and is threaded through nearly every line,
    // so colouring it marks the whole function and costs a slot in a palette of six. What
    // a reader needs to follow is the values that differ between call sites.
    (!word.is_empty() && word != "$" && !is_type(word)).then(|| word.to_string())
}

/// The name a screenshot is rendered under before it is moved into place.
///
/// The suffix goes BEFORE the extension on purpose: `silicon` picks the syntax from the
/// LAST one (`silicon.rs:143`, where Solidity is deliberately highlighted as JavaScript
/// for the Dracula palette), so `fn_x.js.part123` is highlighted as Rust instead and every
/// screenshot in the run comes out a different colour. That shipped in 0.26.24.
fn partial_render_name(file_name: &str, pid: u32) -> String {
    format!("{}.part{pid}.js", file_name.trim_end_matches(".js"))
}

/// The frame's own background says what it holds: red when something drawn here changes
/// state, amber when something here probably does, red winning when both are true — a
/// frame that changes state IS one, whatever else it also might do. A cluster is thirty
/// frames and "where does state change" is asked from a distance where a node's own
/// border is two pixels wide.
fn frame_fill_for(nodes: &[GraphNode]) -> Option<&'static str> {
    if nodes.iter().any(|n| n.writes_storage || !n.write_call_lines.is_empty()) {
        Some(FRAME_FILL_WRITES)
    } else if nodes.iter().any(|n| !n.external_call_lines.is_empty()) {
        Some(FRAME_FILL_MAY_WRITE)
    } else {
        None
    }
}

/// Plan a WHOLE deployment: every frame it will draw, in reading order, each with its
/// geometry already decided and nothing touched on the board. This is the single walk
/// of §21 — the drawing half decides nothing, it consumes this.
fn plan_cluster(
    metadata: &EvmBatMetadata,
    contract_name: &str,
    function_name: &str,
    root_file: &str,
    options: &AutoDeployOptions,
    allocator: &mut ShelfAllocator,
) -> Result<Vec<FramePlan>> {
    let mut plans: Vec<FramePlan> = Vec::new();
    let mut framed: HashSet<String> = HashSet::new();
    plan_subtree(
        metadata,
        contract_name,
        function_name,
        root_file,
        options,
        allocator,
        0,
        &mut framed,
        &mut plans,
        None,
    )?;
    Ok(plans)
}

/// One step of that walk: plan this frame, then everything it cards, depth-first and in
/// call order. A function already planned is NOT planned again — it only gains another
/// way back — which is what makes a helper two branches reach one frame with two
/// references instead of two copies.
#[allow(clippy::too_many_arguments)]
fn plan_subtree(
    metadata: &EvmBatMetadata,
    contract_name: &str,
    function_name: &str,
    root_file: &str,
    options: &AutoDeployOptions,
    allocator: &mut ShelfAllocator,
    cluster_depth: usize,
    framed: &mut HashSet<String>,
    plans: &mut Vec<FramePlan>,
    origin: Option<&str>,
) -> Result<()> {
    let title = format!("{contract_name}.{function_name}");
    if let Some(existing) = plans.iter_mut().find(|plan| plan.title == title) {
        if let Some(origin) = origin {
            existing.origins.push(origin.to_string());
        }
        println!("  {} is already in the plan", title.blue());
        return Ok(());
    }

    let Some(plan) = plan_one(
        metadata,
        contract_name,
        function_name,
        root_file,
        options,
        allocator,
        cluster_depth,
        framed,
    )?
    else {
        return Ok(());
    };
    // Before its children are planned, so a child that calls back into it cards it
    // rather than drawing a second copy.
    framed.insert(title.clone());
    let cards = plan.card_order.clone();
    let index = plans.len();
    plans.push(plan);
    if let Some(origin) = origin {
        plans[index].origins.push(origin.to_string());
    }

    for (target, target_file) in cards {
        let Some((contract, function)) = target.split_once('.') else {
            continue;
        };
        plan_subtree(
            metadata,
            contract,
            function,
            &target_file,
            options,
            allocator,
            cluster_depth + 1,
            framed,
            plans,
            Some(&title),
        )?;
    }
    Ok(())
}

/// Everything the LOCAL half of a deploy computes for one frame: the graph, its layout,
/// its measured size and the slot it takes on the board. Nothing in here needs the
/// network, and the drawing half only reads it — which is what makes it a plan. See §21
/// of docs/diagram-deploy-design.md.
struct FramePlan {
    title: String,
    /// Its level in the cluster, which is its indent on the board.
    cluster_depth: usize,
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
    anchors: Vec<RelativeAnchor>,
    layout: GraphLayout,
    frame_x: f64,
    frame_y: f64,
    /// The frames this one cards, in the order a reader meets them.
    card_order: Vec<(String, String)>,
    /// Every frame of this deployment that cards this one. A frame reached from three
    /// callers carries three ways back, so this is a list, not an option.
    origins: Vec<String>,
}


/// The BOARD half: everything from here on talks to Miro. It reads the plan and
/// changes nothing in it.
async fn draw_one(
    plan: &FramePlan,
    // The frame created for this plan, already on the board and empty.
    frame_id: &str,
    // The entry point this deployment belongs to, which every frame of it records.
    cluster_root: &str,
    // Every frame of this deployment, by title. A card's destination is looked up here,
    // so no frame ever waits for another to be drawn before it can point at it.
    urls: &HashMap<String, String>,
    options: &AutoDeployOptions,
    client: &MiroClient,
) -> Result<()> {
    let FramePlan {
        title,
        cluster_depth: _,
        nodes,
        edges,
        anchors,
        layout,
        frame_x,
        frame_y,
        card_order: _,
        origins,
    } = plan;
    let (frame_x, frame_y) = (*frame_x, *frame_y);
    let by_id: HashMap<&str, &GraphNode> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    // Every deploy is fresh, so nothing is ever reused from the board: this stays empty.
    // It is the hook `--refresh-links` used to come in through, kept so the upload path
    // below reads the same as it did.
    let reuse: HashMap<String, (String, u32, u32)> = HashMap::new();
    let frame_id = frame_id.to_string();
    println!("\n{} {}", "▸".blue(), title.bold());

    // Every card already knows where it goes: the whole deployment's frames exist before
    // any of them is filled, so a destination is a lookup, never a wait.
    let target_frames = urls;

    let mut back_cards: Vec<(String, String)> = Vec::new();
    // The way back. A frame reached from a card carries one card per origin — the plan
    // recorded every frame of this deployment that cards it, so a helper three branches
    // reach gets three. Drawn in the corner the content does not reach, and skipped
    // rather than drawn over code when it does.
    let occupied: Vec<(f64, f64, f64, f64)> = layout
        .nodes
        .iter()
        .map(|placed| (placed.x, placed.y, placed.width, placed.height))
        .collect();
    for (index, origin_title) in origins.iter().enumerate() {
        let Some(origin_url) = urls.get(origin_title) else {
            continue;
        };
        match back_card_slot(layout.frame_width, layout.frame_height, &occupied, index) {
            Some((x, y)) => {
                let id = client
                    .create_back_card(
                        &frame_id,
                        &format!("↑ {origin_title}"),
                        origin_url,
                        x,
                        y,
                        LINK_CARD_WIDTH,
                        LINK_CARD_HEIGHT,
                    )
                    .await
                    .change_context(EvmMiroError)?;
                back_cards.push((origin_title.clone(), id));
            }
            None => println!(
                "  {} no room for the way back to {}; it is in the registry",
                "note:".yellow(),
                origin_title
            ),
        }
    }

    // Record the frame before filling it, so a run that dies partway through
    // still leaves something that names what is on the board.
    let frame_url = client.frame_url(&frame_id);
    let mut record = AutoDeployedFrame {
        entry_point: title.clone(),
        back_cards,
        type_frame: false,
        frame_id: frame_id.clone(),
        frame_url: frame_url.clone(),
        x: frame_x,
        y: frame_y,
        width: layout.frame_width,
        height: layout.frame_height,
        images: Vec::new(),
        image_dims: Vec::new(),
        node_positions: Vec::new(),
        callee_connectors: Vec::new(),
        link_cards: Vec::new(),
        connector_ids: Vec::new(),
        marker_ids: Vec::new(),
        border_ids: Vec::new(),
        // A brand-new frame carries no declaration screenshots: the ones added with
        // `bat-cli screenshot` belong to the frame they were placed on, which this
        // deploy leaves untouched on the board.
        screenshots: Vec::new(),
        // Every frame belongs to the named entry point's cluster, so the next deploy
        // of that entry point can find and report the whole previous cluster.
        cluster_root: cluster_root.to_string(),
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
            NodeKind::Link { target, .. } => target_frames.get(target).cloned().unwrap_or_default(),
            NodeKind::Screenshot => String::new(),
        };
        let reused_image = reuse.get(&node.id).map(|(image_id, _, _)| image_id.clone());
        let (x, y, width, height) = (placed.x, placed.y, placed.width, placed.height);
        let bar = bar.clone();
        upload_tasks.spawn(async move {
            let result = match kind {
                NodeKind::Link { .. } => {
                    client
                        .create_link_card(&frame_id, &label, &target_url, x, y, width, height)
                        .await
                }
                NodeKind::Screenshot => match reused_image {
                    // Refresh: the image is already on the board — just move it.
                    Some(image_id) => client
                        .update_item_position(&image_id, x, y)
                        .await
                        .map(|_| image_id),
                    None => {
                        client
                            .create_image_in_frame(&png_path, &frame_id, &label, x, y, width)
                            .await
                    }
                },
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

    // Storage-write markers: a hollow red rectangle around every node whose function
    // changes state — whether it holds the assignment or only reaches one through what it
    // calls. One marking, not two: the question a reader asks is "does this change state",
    // and a pass-through like `DebtToken.mint` answers yes even though it assigns nothing.
    let drawn_screens = drawn_screen_ids(&nodes);
    let frame_writes: Vec<&GraphNode> = nodes
        .iter()
        .filter(|n| n.writes_storage || surviving_write_calls(n, &drawn_screens).next().is_some())
        .collect();
    let frame_external: Vec<&GraphNode> = nodes
        .iter()
        .filter(|n| !n.external_call_lines.is_empty())
        .collect();
    let borders: Vec<(f64, f64, f64, f64)> = nodes
        .iter()
        .filter(|n| {
            n.writes_storage || surviving_write_calls(n, &drawn_screens).next().is_some()
        })
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
        if node.png_height == 0 {
            continue;
        }
        let surviving: Vec<usize> = surviving_write_calls(node, &drawn_screens)
            .map(|(line, _, _)| *line)
            .collect();
        if node.write_lines.is_empty() && surviving.is_empty() {
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
            .chain(surviving.iter().copied())
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
        /// The x of this arrow's own vertical lane in the gutter, and the y it has to
        /// reach. Miro routes a connector itself — you give it two endpoints and it
        /// picks the path — so the only way to stop every arrow leaving a caller from
        /// turning on the same x is to stop asking Miro to route at all: the arrow is
        /// drawn as straight segments between markers we place, one lane per arrow.
        lane_x: f64,
        end_y: f64,
        /// The callee's graph node id, so the connectors drawn for it can be
        /// attributed to it for surgical removal.
        node_id: String,
        end_anchor: RelativeAnchor,
        end_point: (f64, f64),
        /// This callee's own colour. A caller line can call several functions
        /// (`_usd(_token(x))`); they share one stub, but each branch keeps the colour
        /// of the function it reaches, so two callees never read as one.
        color: String,
        /// How this callee's line is drawn. Dotted when the gutter ran out of hues and
        /// the colour had to repeat, so the two are still two arrows.
        stroke: ConnectorStroke,
    }
    struct PendingGroup {
        token_x: f64,
        token_y: f64,
        edge_x: f64,
        exit_right: bool,
        style: ConnectorStyle,
        callees: Vec<CalleeLink>,
    }

    // One vertical lane per forward arrow, inside the gutter between the two columns
    // it spans. Lanes are ordered by where the arrow starts and where it ends, which
    // is what keeps two arrows that do not have to cross from crossing: for two
    // arrows going the same way, the one starting lower takes the outer lane, so each
    // one's horizontal leg passes outside the other's vertical leg instead of through
    // it. Arrows that do have to cross (their start and end order disagree) cross
    // once, which is unavoidable with one box per function.
    let lane_of: HashMap<(String, String, usize), f64> = {
        let mut per_layer_right: HashMap<usize, f64> = HashMap::new();
        let mut per_layer_left: HashMap<usize, f64> = HashMap::new();
        for placed in &layout.nodes {
            let right = placed.x + placed.width / 2.0;
            let left = placed.x - placed.width / 2.0;
            per_layer_right
                .entry(placed.layer)
                .and_modify(|value| *value = value.max(right))
                .or_insert(right);
            per_layer_left
                .entry(placed.layer)
                .and_modify(|value| *value = value.min(left))
                .or_insert(left);
        }
        // Group the forward edges by the gutter they cross, carrying (start y, end y).
        let mut per_gap: HashMap<usize, Vec<((String, String, usize), f64, f64)>> = HashMap::new();
        for edge in edges.iter() {
            let (Some(from), Some(to)) = (layout.node(&edge.from), layout.node(&edge.to)) else {
                continue;
            };
            if to.layer <= from.layer {
                continue; // a cycle: drawn dashed, and left to Miro
            }
            let start_y = from.y;
            let end_y = to.y;
            per_gap.entry(from.layer).or_default().push((
                (edge.from.clone(), edge.to.clone(), edge.line_in_slice),
                start_y,
                end_y,
            ));
        }
        let mut lanes = HashMap::new();
        for (layer, mut arrows) in per_gap {
            let right = per_layer_right.get(&layer).copied().unwrap_or(0.0);
            let left = per_layer_left
                .get(&(layer + 1))
                .copied()
                .unwrap_or(right + 550.0);
            // Keep the first turn clear of the screenshot border (and of the red or
            // amber band drawn on it), and give each lane five stroke widths of air —
            // narrowing only when the gutter cannot hold them all.
            let margin = 100.0_f64.min((left - right) / 4.0);
            let usable = (left - right - 2.0 * margin).max(0.0);
            let pitch = if arrows.len() > 1 {
                (usable / (arrows.len() - 1) as f64).min(LANE_PITCH)
            } else {
                0.0
            };
            arrows.sort_by(|a, b| {
                let down = |start: f64, end: f64| end >= start;
                let (a_down, b_down) = (down(a.1, a.2), down(b.1, b.2));
                // Arrows going up take the inner lanes, ordered by where they start;
                // arrows going down take the outer ones, the lowest start furthest out.
                a_down
                    .cmp(&b_down)
                    .then_with(|| {
                        let key = |arrow: &((String, String, usize), f64, f64)| {
                            if a_down { (-arrow.1, -arrow.2) } else { (arrow.1, arrow.2) }
                        };
                        key(a).partial_cmp(&key(b)).unwrap_or(std::cmp::Ordering::Equal)
                    })
            });
            for (index, (key, _, _)) in arrows.into_iter().enumerate() {
                lanes.insert(key, right + margin + index as f64 * pitch);
            }
        }
        lanes
    };

    // Colour the ARROWS, by colouring a conflict graph rather than by ranking boxes.
    //
    // What a colour is for is telling two arrows apart where a reader has to compare
    // them, and that is two places only: arrows running side by side in the same
    // gutter, and arrows leaving the same screenshot. Every rank-based rule collided
    // somewhere else — ranking by depth put the same colour on the two arrows most
    // likely to be side by side, ranking by column position left repeats a palette
    // apart that can still end up in neighbouring lanes. So: build the conflicts, walk
    // the arrows in a fixed order (gutter, then lane), and give each the first colour
    // none of its conflicting neighbours already has. Two arrows that reach the SAME
    // function are not in conflict — sharing their colour is what lets a reader
    // recognise a helper drawn in several places.
    let edge_color: (Vec<String>, HashMap<String, ConnectorStroke>) = {
        let lane_index: HashMap<usize, (usize, f64)> = edges
            .iter()
            .enumerate()
            .filter_map(|(index, edge)| {
                let from = layout.node(&edge.from)?;
                let lane = lane_of.get(&(edge.from.clone(), edge.to.clone(), edge.line_in_slice))?;
                Some((index, (from.layer, *lane)))
            })
            .collect();
        // A fixed walk order, so the same graph always comes out the same colours.
        let mut order: Vec<usize> = (0..edges.len()).collect();
        order.sort_by(|a, b| {
            let key = |index: &usize| lane_index.get(index).copied().unwrap_or((usize::MAX, 0.0));
            let (la, xa) = key(a);
            let (lb, xb) = key(b);
            la.cmp(&lb)
                .then(xa.partial_cmp(&xb).unwrap_or(std::cmp::Ordering::Equal))
                .then(a.cmp(b))
        });
        // Neighbours in the gutter: the two lanes on either side are what a reader's
        // eye actually puts next to each other.
        const LANE_NEIGHBOURHOOD: usize = 2;
        let mut per_gap: HashMap<usize, Vec<usize>> = HashMap::new();
        for index in &order {
            if let Some((layer, _)) = lane_index.get(index) {
                per_gap.entry(*layer).or_default().push(*index);
            }
        }
        let mut conflicts: Vec<HashSet<usize>> = vec![HashSet::new(); edges.len()];
        for arrows in per_gap.values() {
            for (position, index) in arrows.iter().enumerate() {
                let lower = position.saturating_sub(LANE_NEIGHBOURHOOD);
                let upper = (position + LANE_NEIGHBOURHOOD + 1).min(arrows.len());
                for other in &arrows[lower..upper] {
                    if other != index && edges[*other].to != edges[*index].to {
                        conflicts[*index].insert(*other);
                        conflicts[*other].insert(*index);
                    }
                }
            }
        }
        // Leaving the same screenshot: those the reader compares directly, at the call
        // lines. And ARRIVING at boxes that sit next to each other in the next column:
        // their last horizontal legs run parallel, a stone's throw apart, which is the
        // same comparison at the other end of the arrow.
        for (index, edge) in edges.iter().enumerate() {
            for (other, sibling) in edges.iter().enumerate() {
                if other != index && sibling.from == edge.from && sibling.to != edge.to {
                    conflicts[index].insert(other);
                }
            }
        }
        let mut column: HashMap<usize, Vec<(f64, String)>> = HashMap::new();
        for placed in &layout.nodes {
            column
                .entry(placed.layer)
                .or_default()
                .push((placed.y, placed.id.clone()));
        }
        let mut neighbour_of: HashMap<&str, HashSet<String>> = HashMap::new();
        for boxes in column.values_mut() {
            boxes.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            for (position, (_, id)) in boxes.iter().enumerate() {
                let lower = position.saturating_sub(1);
                let upper = (position + 2).min(boxes.len());
                neighbour_of.insert(
                    id.as_str(),
                    boxes[lower..upper]
                        .iter()
                        .filter(|(_, other)| other != id)
                        .map(|(_, other)| other.clone())
                        .collect(),
                );
            }
        }
        for (index, edge) in edges.iter().enumerate() {
            let Some(neighbours) = neighbour_of.get(edge.to.as_str()) else {
                continue;
            };
            for (other, sibling) in edges.iter().enumerate() {
                if other != index && neighbours.contains(&sibling.to) {
                    conflicts[index].insert(other);
                    conflicts[other].insert(index);
                }
            }
        }
        // Colour the CALLEES, not the arrows one at a time.
        //
        // Every arrow to one function shares its colour, which is what lets a reader
        // recognise a helper drawn in several places — but colouring arrow by arrow and
        // then locking the callee's colour on first sight meant a later arrow to that
        // callee inherited a colour WITHOUT checking where it now runs. Two crimson
        // arrows ended up side by side in one gutter, 15px apart, indistinguishable. So
        // the conflicts are collapsed onto the callees first: two functions conflict when
        // ANY of their arrows are compared anywhere, and the colouring is over that.
        let mut callee_conflicts: HashMap<&str, HashSet<&str>> = HashMap::new();
        for (index, edge) in edges.iter().enumerate() {
            let entry = callee_conflicts.entry(edge.to.as_str()).or_default();
            for other in &conflicts[index] {
                let rival = edges[*other].to.as_str();
                if rival != edge.to.as_str() {
                    entry.insert(rival);
                }
            }
        }
        let first_gutter: HashMap<&str, usize> = {
            let mut first: HashMap<&str, usize> = HashMap::new();
            for index in &order {
                if let Some((gutter, _)) = lane_index.get(index) {
                    first.entry(edges[*index].to.as_str()).or_insert(*gutter);
                }
            }
            first
        };
        let mut palette_of: HashMap<usize, Vec<usize>> = HashMap::new();
        let by_callee = color_callees(&callee_conflicts, |callee| {
            // Each gutter tries the palette in its own SHUFFLED order. The conflict rule
            // alone always reached for colour 0 first, so every column opened on the same
            // hue and a frame read as one repeated pattern — correct, and monotonous. The
            // shuffle is drawn per run: a deploy draws a fresh cluster anyway, so there is
            // nothing to keep stable between runs.
            let gutter = first_gutter.get(callee).copied().unwrap_or(usize::MAX);
            palette_of
                .entry(gutter)
                .or_insert_with(|| {
                    let mut order: Vec<usize> = (0..DEPTH_COLORS.len()).collect();
                    order.shuffle(&mut rand::thread_rng());
                    order
                })
                .clone()
        });
        let strokes: HashMap<String, ConnectorStroke> = by_callee
            .iter()
            .map(|(callee, (_, stroke))| (callee.to_string(), *stroke))
            .collect();
        let colors: Vec<String> = edges
            .iter()
            .map(|edge| {
                let (color, _) = by_callee.get(edge.to.as_str()).copied().unwrap_or_default();
                DEPTH_COLORS[color].to_string()
            })
            .collect();
        (colors, strokes)
    };
    let (edge_color, callee_stroke) = edge_color;
    let callee_color: HashMap<String, String> = edges
        .iter()
        .enumerate()
        .map(|(index, edge)| (edge.to.clone(), edge_color[index].clone()))
        .collect();

    let mut groups: HashMap<(String, usize, bool), PendingGroup> = HashMap::new();
    for edge in edges.iter() {
        let edge_key = (edge.from.clone(), edge.to.clone(), edge.line_in_slice);
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
            lane_x: lane_of.get(&edge_key).copied().unwrap_or(f64::NAN),
            end_y: end_point.1,
            node_id: edge.to.clone(),
            end_anchor: RelativeAnchor::new(if exit_right { 0.0 } else { 1.0 }, callee_fraction),
            end_point,
            color: callee_color
                .get(&edge.to)
                .cloned()
                .unwrap_or_else(|| DEPTH_COLORS[0].to_string()),
            stroke: callee_stroke.get(&edge.to).copied().unwrap_or_default(),
        };

        let stroke = if back_edges.contains(&(edge.from.clone(), edge.to.clone())) {
            ConnectorStroke::Dashed
        } else {
            ConnectorStroke::Solid
        };
        let group = groups
            .entry((edge.from.clone(), edge.line_in_slice, exit_right))
            .or_insert_with(|| {
                // The shared arrow anchor for this caller line + side: the arrow lands
                // at the END of the line for a right entry, or its START for a left
                // one, and enters by a straight horizontal stub from the edge.
                let anchor = line_anchor(
                    (
                        caller_placed.x,
                        caller_placed.y,
                        caller_placed.width,
                        caller_placed.height,
                    ),
                    caller.png_width,
                    caller.png_height,
                    caller.font_size,
                    &caller.rendered_lines,
                    caller.line_offset,
                    edge.line_in_slice.saturating_sub(1) + PATH_HEADER_LINES,
                    exit_right,
                );
                let (token_x, token_y, edge_x) = (anchor.token_x, anchor.token_y, anchor.edge_x);
                PendingGroup {
                    token_x,
                    token_y,
                    edge_x,
                    exit_right,
                    style: ConnectorStyle {
                        stroke_color: callee_color
                            .get(&edge.to)
                            .cloned()
                            .unwrap_or_else(|| DEPTH_COLORS[0].to_string()),
                        stroke_width: options.stroke_width.to_string(),
                        stroke: ConnectorStroke::Solid,
                        caption: None,
                        arrow: ArrowEnd::Start,
                    },
                    callees: Vec::new(),
                }
            });
        group.style.stroke = group.style.stroke.strongest(stroke);
        group.callees.push(link);
    }

    let bar = phase_bar("drawing connectors", groups.len());
    let mut connector_tasks = tokio::task::JoinSet::new();
    for (_key, group) in groups {
        let client = client.clone();
        let frame_id = frame_id.clone();
        let bar = bar.clone();
        // Attribute this whole group's connectors + markers to its primary callee,
        // so removing that callee later deletes exactly its arrows.
        let owner = group.callees.first().map(|link| link.node_id.clone());
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
            // The shared stub exists for the arrows Miro still routes. A lane-routed
            // arrow reaches the call line by itself, and drawing the stub as well put
            // a second horizontal on the same y — the little hook the reader sees at
            // the caller's edge is that duplicate, plus the elbow Miro adds to join
            // them.
            let lane_routed = group.exit_right && group.callees.iter().all(|link| link.lane_x.is_finite());
            if !lane_routed {
                let mut stub_style = group.style.clone();
                stub_style.arrow = ArrowEnd::End;
                connectors.push(
                    client
                        .create_connector(&edge_marker, edge_side, &token_marker, token_side, stub_style)
                        .await?,
                );
            }

            for link in &group.callees {
                let mut route_style = group.style.clone();
                route_style.arrow = ArrowEnd::None;
                route_style.stroke_color = link.color.clone();
                route_style.stroke = route_style.stroke.strongest(link.stroke);
                // Without a lane (a cycle, drawn dashed, or a gutter too narrow to
                // hold one) fall back to the single Miro-routed connector.
                if !group.exit_right || !link.lane_x.is_finite() {
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
                    continue;
                }
                // Three straight legs, each between two points that share an axis, so
                // there is nothing left for Miro to route: out of the caller at the
                // call line, down (or up) this arrow's own lane, into the callee's
                // signature line.
                let lane_top = client
                    .create_anchor_marker(&frame_id, link.lane_x, group.token_y, ANCHOR_MARKER_SIZE)
                    .await?;
                let lane_end = client
                    .create_anchor_marker(&frame_id, link.lane_x, link.end_y, ANCHOR_MARKER_SIZE)
                    .await?;
                markers.push(lane_top.clone());
                markers.push(lane_end.clone());
                // Straight into the call line itself, carrying this arrow's head: the
                // edge marker is not on the path at all.
                let mut head_style = route_style.clone();
                head_style.arrow = ArrowEnd::End;
                connectors.push(
                    client
                        .create_connector(
                            &lane_top,
                            RelativeAnchor::new(0.0, 0.5),
                            &token_marker,
                            RelativeAnchor::new(1.0, 0.5),
                            head_style,
                        )
                        .await?,
                );
                let (top_side, end_side) = if link.end_y >= group.token_y {
                    (RelativeAnchor::new(0.5, 1.0), RelativeAnchor::new(0.5, 0.0))
                } else {
                    (RelativeAnchor::new(0.5, 0.0), RelativeAnchor::new(0.5, 1.0))
                };
                connectors.push(
                    client
                        .create_connector(&lane_top, top_side, &lane_end, end_side, route_style.clone())
                        .await?,
                );
                connectors.push(
                    client
                        .create_connector(
                            &link.end_id,
                            link.end_anchor,
                            &lane_end,
                            RelativeAnchor::new(1.0, 0.5),
                            route_style,
                        )
                        .await?,
                );
            }

            bar.inc(1);
            Ok::<_, error_stack::Report<crate::batbelt::miro::MiroError>>((owner, markers, connectors))
        });
    }

    let mut connector_ids = Vec::new();
    let mut marker_ids = Vec::new();
    let mut callee_owned: HashMap<String, Vec<String>> = HashMap::new();
    while let Some(joined) = connector_tasks.join_next().await {
        let (owner, markers, connectors) = joined
            .into_report()
            .change_context(EvmMiroError)?
            .change_context(EvmMiroError)?;
        if let Some(owner) = owner {
            let bucket = callee_owned.entry(owner).or_default();
            bucket.extend(markers.iter().cloned());
            bucket.extend(connectors.iter().cloned());
        }
        marker_ids.extend(markers);
        connector_ids.extend(connectors);
    }
    bar.finish_and_clear();
    println!("    {} {} connector(s)", "✓".green(), connector_ids.len());

    // Store each screenshot's measured size so a later --refresh-links can reuse
    // the uploaded image without re-rendering.
    record.image_dims = nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Screenshot && image_ids.contains_key(&node.id))
        .map(|node| (node.id.clone(), node.png_width, node.png_height))
        .collect();
    // Positions and per-callee connector ownership for a surgical --refresh-links.
    record.node_positions = nodes
        .iter()
        .filter_map(|node| layout.node(&node.id).map(|placed| (node.id.clone(), placed.x, placed.y)))
        .collect();
    record.callee_connectors = callee_owned.into_iter().collect();
    // Record link cards by the TARGET they stand for (their own node id is a
    // throwaway `\0link{n}` that changes every deploy), so a later --refresh-links
    // recognises them and never re-creates or deletes them.
    record.link_cards = nodes
        .iter()
        .filter_map(|node| match &node.kind {
            NodeKind::Link { .. } => {
                let target_id = node.label.replacen('.', "::", 1);
                image_ids
                    .get(&node.id)
                    .map(|card_id| (target_id, card_id.clone(), String::new()))
            }
            NodeKind::Screenshot => None,
        })
        .collect();
    record.images = image_ids.into_iter().collect();
    record.connector_ids = connector_ids;
    record.marker_ids = marker_ids;
    save_frame_record(&record)?;

    println!("  {}", frame_url.blue());
    // NB: screenshot files are NOT deleted here — they are shared across every frame
    // in this run (see render_and_measure), so the whole figures folder is wiped once
    // at the end of `run()`.
    Ok(())
}

/// Store what a deployment owns, replacing any earlier record for the same
/// entry point.
/// A record belongs to a DEPLOYMENT, and a deployment is an entry point.
///
/// The registry used to hold one record per function name, board-wide, which was true
/// while a callee already on the board was linked rather than redrawn. Now that every
/// deploy is fresh, a helper cut to its own frame is drawn once per deployment: the
/// board carries several frames titled `auto: FLAMMFlowLib.requireFlat`, one per entry
/// point that reaches it, with different ids and the same title. Keyed by name alone,
/// the second deployment's record displaced the first's and the first's frames became
/// unreachable from the CLI although they were right there on the board.
///
/// So the key is (deployment, frame), where the deployment is `cluster_root` — the
/// entry point the whole cluster was drawn for — and the frame is the function it
/// shows. Re-deploying an entry point replaces that deployment's records and no other.
pub(crate) fn save_frame_record(record: &AutoDeployedFrame) -> Result<()> {
    let record = record.clone();
    EvmBatMetadata::update_metadata(move |metadata| {
        metadata.miro.auto.frames.retain(|frame| {
            frame.cluster_root != record.cluster_root || frame.entry_point != record.entry_point
        });
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
/// Is this contract one the auditor said they already know?
///
/// The density of a diagram is not evenly useful. A fixed-point math library called
/// from thirty places is thirty boxes that say the same thing, and the reader knew
/// what `mulDiv` did before they opened the board. Naming it here removes its boxes
/// and its arrows — not the calls to it, which stay visible in the callers' own
/// screenshots, so nothing about the audited code is hidden. Deploy it as an entry
/// point of its own on the day the question is actually about it.
fn ignored_contract(options: &AutoDeployOptions, contract: &ContractMetadata) -> bool {
    options
        .ignore_contracts
        .iter()
        .any(|pattern| matches_ignore(pattern, &contract.name, &contract.file_path))
}

/// Does this ignore pattern mean this contract?
///
/// A name matches exactly, and a path matches whole SEGMENTS. The substring match this
/// replaces was a trap: `ignore Math` also hid `CollRebalancerMath`, because its path is
/// `.../lev/CollRebalancerMath.sol` and that contains "Math". A call to it then vanished
/// from a diagram completely — no box, no card, no marking — and the deploy's only word on
/// the subject was "not drawing: Math". Whatever an auditor means by naming a library they
/// have read, they do not mean every contract whose file name happens to end in it.
pub(crate) fn matches_ignore(pattern: &str, name: &str, file_path: &str) -> bool {
    let pattern = pattern.trim().trim_matches('/');
    if pattern.is_empty() {
        return false;
    }
    if name == pattern {
        return true;
    }
    // A path pattern is one or more whole segments: `utils/math` matches
    // `lib/oz/contracts/utils/math/Math.sol`, and `Math` matches a directory or file
    // called exactly that, never a longer name containing it.
    let wanted: Vec<&str> = pattern.split('/').filter(|part| !part.is_empty()).collect();
    let segments: Vec<&str> = file_path
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .map(|part| part.strip_suffix(".sol").unwrap_or(part))
        .collect();
    !wanted.is_empty() && segments.windows(wanted.len()).any(|window| window == wanted)
}

fn build_graph(
    metadata: &EvmBatMetadata,
    contract_name: &str,
    function_name: &str,
    // The file the root contract lives in, so a name shared by several contracts (three
    // vendored copies of `BeaconProxy`) resolves to the one that was actually selected.
    root_file: &str,
    options: &AutoDeployOptions,
) -> Result<(
    Vec<GraphNode>,
    Vec<GraphEdge>,
    Vec<crate::batbelt::evm::metadata::bat_metadata::UnresolvedCall>,
)> {
    let Some((root_contract, root_function)) =
        find_function(metadata, contract_name, root_file, function_name, None)
    else {
        return Ok((Vec::new(), Vec::new(), Vec::new()));
    };

    // Method name → in-scope, non-stub contracts that directly define it, built
    // ONCE so resolve_call's unique-definer fallback is O(1) per call site instead
    // of scanning every contract each time.
    let mut definer_map: HashMap<String, Vec<String>> = HashMap::new();
    for contract in &metadata.contracts {
        if contract.contract_type == EvmContractType::Interface {
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

    // Whether a call changes state is a fact about the code, so the reachability walk
    // always crosses into `lib/` — independently of whether this deploy draws it.
    let write_options = options.clone();
    let mut write_definer_map: HashMap<String, Vec<String>> = HashMap::new();
    for contract in &metadata.contracts {
        if contract.contract_type == EvmContractType::Interface {
            continue;
        }
        for function in &contract.functions {
            if !function.is_stub {
                write_definer_map
                    .entry(function.name.clone())
                    .or_default()
                    .push(contract.name.clone());
            }
        }
    }
    let mut write_memo: HashMap<String, bool> = HashMap::new();
    let mut external_memo: HashMap<String, bool> = HashMap::new();

    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();
    // Interface calls in this tree still needing an AI resolution (see `resolve`).
    // Reads through an interface cast with several in-scope implementations, left out of
    // the graph: `(interface, method) → (candidates, call locations)`, printed once each.
    let mut left_out_reads: std::collections::BTreeMap<(String, String), (Vec<String>, Vec<String>)> =
        std::collections::BTreeMap::new();
    let mut unresolved: Vec<crate::batbelt::evm::metadata::bat_metadata::UnresolvedCall> =
        Vec::new();
    // Calls dropped because their contract is on the ignore list. Leaving a call out is a
    // reasonable thing to ask for and an unreasonable thing to do in silence: the diagram
    // cannot show what is not there, so the run says it.
    let mut skipped_by_ignore: HashMap<String, usize> = HashMap::new();
    // One node per function: a second call to the same function points at the
    // node that already exists.
    let mut drawn: HashMap<String, String> = HashMap::new();

    let root_id = overload_node_key(root_contract, &root_function);
    drawn.insert(root_id.clone(), root_id.clone());
    let mut root_node = make_node(
        root_id.clone(),
        format!("{contract_name}.{function_name}"),
        root_contract,
        &root_function,
        0,
        doc_lines_above(options, &root_contract.file_path, root_function.line),
    );
    root_node.leads_to_write = leads_to_write(
        metadata,
        root_contract,
        &root_function,
        &write_options,
        &write_definer_map,
        &mut write_memo,
        &mut HashSet::new(),
    );
    nodes.push(root_node);

    // Depth-first, so siblings stay in source order and each subtree is built
    // before the next one starts — which is the order the layout wants. `line`
    // pins WHICH overload this node is, so the re-read below lands on the same one.
    struct Pending {
        node_id: String,
        contract: String,
        // The file of that exact contract. A name alone is not an identity once `lib/`
        // vendors several copies of a library.
        file: String,
        function: String,
        line: usize,
        depth: usize,
    }

    let mut stack = vec![Pending {
        node_id: root_id,
        contract: root_contract.name.clone(),
        file: root_contract.file_path.clone(),
        function: function_name.to_string(),
        line: root_function.line,
        depth: 0,
    }];

    while let Some(current) = stack.pop() {
        // The contract that defines the function, which is where its source is.
        // Pin the exact overload by line so a re-read never drifts to a sibling.
        let Some((contract, function)) =
            find_function_at(
                metadata,
                &current.contract,
                &current.file,
                &current.function,
                current.line,
            )
        else {
            continue;
        };
        let slice = read_slice(
            &contract.file_path,
            function.line,
            function_end(&function, contract),
        );
        // `slice` is the body, so every line found in it is relative to the declaration.
        // The screenshot may start higher (the NatSpec), so shift the anchors by exactly
        // the number of documentation lines drawn above this caller.
        let doc_shift = doc_lines_above(options, &contract.file_path, function.line);

        let mut children: Vec<Pending> = Vec::new();
        // Lines in THIS function whose call reaches a storage write, collected while the
        // calls are resolved and written back onto the node once the loop is done.
        let mut caller_write_calls: Vec<(usize, String, String)> = Vec::new();
        let mut caller_external_calls: Vec<(usize, String, String)> = Vec::new();
        // Lines whose call leaves the drawn graph towards `lib/` code that can mutate.
        let mut lib_boundary_lines: Vec<usize> = Vec::new();

        // Modifiers count as dependencies; their call site is the line of the
        // signature where the modifier name appears.
        for modifier_name in &function.modifiers {
            // A base constructor invoked in a constructor's header (`BeaconProxy(beacon,
            // data)`) is parsed as a modifier too. It is not one, so it resolves to nothing
            // here and is drawn below, as the constructor call it really is.
            let Some((owner, definition)) =
                find_modifier(metadata, &current.contract, &current.file, modifier_name)
            else {
                continue;
            };
            let line_in_slice = slice
                .iter()
                .position(|line| line_has_token(line, modifier_name))
                .map(|index| index + 1)
                .unwrap_or(1);
            let target_id = node_key(&owner.name, &definition.name);
            edges.push(GraphEdge {
                from: current.node_id.clone(),
                to: target_id.clone(),
                line_in_slice: line_in_slice + doc_shift,
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
                    doc_lines_above(options, &owner.file_path, definition.line),
                ));
            }
        }

        for call in extract_call_sites_from_source(&body_only(&slice).join("\n")) {
            let arity = (call.arg_count != usize::MAX).then_some(call.arg_count);

            // Mark the calling line whenever the call reaches a write, BEFORE deciding
            // whether the callee is drawn. A target in `lib/` is dropped from the graph
            // without `--include-external`, but the state change it causes is still real,
            // and this line is the last place in the frame that can show it.
            if let Some((reached_contract, reached_function)) = resolve_call(
                metadata,
                contract,
                &call.name,
                arity,
                &write_options,
                &write_definer_map,
            ) {
                if leads_to_write(
                    metadata,
                    reached_contract,
                    &reached_function,
                    &write_options,
                    &write_definer_map,
                    &mut write_memo,
                    &mut HashSet::new(),
                ) {
                    // Empty when the callee is dropped from the graph (a `lib/` target
                    // without `--include-external`): nothing downstream can carry the
                    // mark, so this line keeps it.
                    let callee_id = resolve_call(
                        metadata,
                        contract,
                        &call.name,
                        arity,
                        options,
                        &definer_map,
                    )
                    .map(|(c, f)| overload_node_key(c, &f))
                    .unwrap_or_default();
                    caller_write_calls.push((
                        function.line + call.line - 1,
                        call.symbol.clone(),
                        callee_id,
                    ));
                } else if leads_to_external(
                    metadata,
                    reached_contract,
                    &reached_function,
                    &write_options,
                    &write_definer_map,
                    &mut external_memo,
                    &mut HashSet::new(),
                ) {
                    // Reaches a boundary but writes no storage this scan can see: amber,
                    // on the same terms as red — the frame that draws the boundary owns
                    // the mark, so this one is kept only if the callee is not drawn here.
                    let callee_id = resolve_call(
                        metadata,
                        contract,
                        &call.name,
                        arity,
                        options,
                        &definer_map,
                    )
                    .map(|(c, f)| overload_node_key(c, &f))
                    .unwrap_or_default();
                    caller_external_calls.push((
                        function.line + call.line - 1,
                        call.symbol.clone(),
                        callee_id,
                    ));
                }
            }

            let Some((target_contract, target_function)) =
                resolve_call(metadata, contract, &call.name, arity, options, &definer_map)
            else {
                // An interface cast with several implementations and no recorded
                // resolution. Guessing would draw code that may never run here, so:
                // - if any candidate can change state, it goes on the list the deploy
                //   stops on, the same list `bat-cli resolve` works through;
                // - if none can, it is a read — stopping a deploy for every
                //   `IERC20(token).balanceOf` is the noise the scan already prunes — so
                //   say it was left out and how to bring it in, and carry on.
                // A call that resolves only because `lib/` was allowed in: the target is
                // dependency code this frame does not draw. A non-view one can change state
                // (`SafeERC20.safeTransferFrom` moves tokens), and it is invisible otherwise
                // — the write happens in a contract that is not even in the repository.
                if let Some((lib_contract, lib_function)) = resolve_call(
                    metadata,
                    contract,
                    &call.name,
                    arity,
                    &write_options,
                    &write_definer_map,
                ) {
                    let read_only = matches!(
                        lib_function.mutability,
                        crate::batbelt::evm::types::EvmMutability::View
                            | crate::batbelt::evm::types::EvmMutability::Pure
                    );
                    if lib_contract.external && !read_only {
                        lib_boundary_lines.push(function.line + call.line - 1);
                    }
                }

                if let Some((receiver, method)) = call.name.split_once('.') {
                    if let Some(type_name) = receiver.strip_suffix("()") {
                        let candidates = cast_implementations(
                            metadata,
                            contract,
                            type_name,
                            method,
                            arity,
                        );
                        if candidates.len() > 1 && !metadata.resolutions.contains_key(type_name) {
                            let names: Vec<String> =
                                candidates.iter().map(|(c, _)| c.name.clone()).collect();
                            let writes = candidates.iter().any(|(c, f)| {
                                leads_to_write(
                                    metadata,
                                    c,
                                    f,
                                    &write_options,
                                    &write_definer_map,
                                    &mut write_memo,
                                    &mut HashSet::new(),
                                )
                            });
                            if writes {
                                unresolved.push(
                                    crate::batbelt::evm::metadata::bat_metadata::UnresolvedCall {
                                        receiver: receiver.to_string(),
                                        method: method.to_string(),
                                        inferred_type: type_name.to_string(),
                                        candidates: names,
                                        assigned_in: Vec::new(),
                                    },
                                );
                            } else {
                                let entry = left_out_reads
                                    .entry((type_name.to_string(), method.to_string()))
                                    .or_insert_with(|| (names.clone(), Vec::new()));
                                entry
                                    .1
                                    .push(format!("{}:{}", contract.name, function.line + call.line - 1));
                            }
                        }
                    }
                }
                continue;
            };
            if ignored_contract(options, target_contract) {
                *skipped_by_ignore.entry(target_contract.name.clone()).or_insert(0) += 1;
                continue;
            }
            let target_id = overload_node_key(target_contract, &target_function);
            if target_id == current.node_id {
                continue; // a function calling itself needs no arrow
            }

            edges.push(GraphEdge {
                from: current.node_id.clone(),
                to: target_id.clone(),
                line_in_slice: call.line + doc_shift,
                column: call.column,
                symbol: call.symbol.clone(),
            });


            // Seen before: the arrow points at the node already drawn, and there
            // is nothing left to expand.
            if drawn.insert(target_id.clone(), target_id.clone()).is_some() {
                continue;
            }
            let mut child = make_node(
                target_id.clone(),
                display_label(target_contract, &target_function),
                target_contract,
                &target_function,
                current.depth + 1,
                doc_lines_above(options, &target_contract.file_path, target_function.line),
            );
            child.leads_to_write = leads_to_write(
                metadata,
                target_contract,
                &target_function,
                &write_options,
                &write_definer_map,
                &mut write_memo,
                &mut HashSet::new(),
            );
            nodes.push(child);
            children.push(Pending {
                node_id: target_id,
                contract: target_contract.name.clone(),
                file: target_contract.file_path.clone(),
                function: target_function.name.clone(),
                line: target_function.line,
                depth: current.depth + 1,
            });
        }

        // A constructor runs its base contracts' constructors before its own body, whether
        // the header invokes them (`BeaconProxy(beacon, data)`) or not. Without this a
        // constructor like `FLAMMProxy`'s — empty, everything it does lives in the base —
        // drew as one lonely screenshot with nothing below it.
        if function.is_constructor {
            let header_end = slice
                .iter()
                .position(|line| line.contains('{'))
                .unwrap_or(0);
            for (base, constructor) in base_constructors(metadata, contract) {
                if ignored_contract(options, base) {
                    continue;
                }
                // Anchor on the header token naming the base when it is invoked there;
                // an implicit base constructor has no call site, so the signature.
                let line_in_slice = slice
                    .iter()
                    .take(header_end + 1)
                    .position(|line| line_has_token(line, &base.name))
                    .map(|index| index + 1)
                    .unwrap_or(1);
                let target_id = overload_node_key(base, &constructor);
                if target_id == current.node_id {
                    continue;
                }
                edges.push(GraphEdge {
                    from: current.node_id.clone(),
                    to: target_id.clone(),
                    line_in_slice: line_in_slice + doc_shift,
                    column: slice
                        .get(line_in_slice - 1)
                        .and_then(|line| line.find(base.name.as_str()))
                        .unwrap_or(0),
                    symbol: base.name.clone(),
                });
                if drawn.insert(target_id.clone(), target_id.clone()).is_some() {
                    continue;
                }
                let mut child = make_node(
                    target_id.clone(),
                    display_label(base, &constructor),
                    base,
                    &constructor,
                    current.depth + 1,
                    doc_lines_above(options, &base.file_path, constructor.line),
                );
                child.leads_to_write = leads_to_write(
                    metadata,
                    base,
                    &constructor,
                    &write_options,
                    &write_definer_map,
                    &mut write_memo,
                    &mut HashSet::new(),
                );
                nodes.push(child);
                children.push(Pending {
                    node_id: target_id,
                    contract: base.name.clone(),
                    file: base.file_path.clone(),
                    function: constructor.name.clone(),
                    line: constructor.line,
                    depth: current.depth + 1,
                });
            }
        }

        // Cross-contract interface calls: follow the ones the AI has resolved (so the
        // concrete downstream function — and its storage writes — appear and recurse),
        // and collect the rest as needing a resolution.
        //
        // Two sources feed the same drawing: interface calls the AI resolved with
        // `bat-cli resolve`, and calls the scan already pinned by type (`resolved_calls`),
        // such as `$.priceFeed.pegOk` — a storage-struct field the deploy cannot type
        // itself. The third element is the originating unresolved call, if any.
        let mut followed: Vec<(String, String, Option<&crate::batbelt::evm::metadata::bat_metadata::UnresolvedCall>)> =
            Vec::new();
        for u in &function.unresolved_calls {
            let concrete = if u.inferred_type.is_empty() {
                None
            } else {
                metadata.resolutions.get(&u.inferred_type)
            };
            match concrete {
                Some(concrete) => followed.push((u.method.clone(), concrete.clone(), Some(u))),
                None => unresolved.push(u.clone()),
            }
        }
        for typed in &function.resolved_calls {
            followed.push((typed.method.clone(), typed.contract.clone(), None));
        }
        for (method, concrete, origin) in &followed {
            let Some((tc, tf)) =
                find_function(metadata, concrete, &contract.file_path, method, None)
            else {
                // A resolution is set but its method isn't there — still unresolved.
                if let Some(u) = origin {
                    unresolved.push((*u).clone());
                }
                continue;
            };
            // A resolution that lands on a virtual/interface stub is redirected to
            // its concrete override; a pure declaration with none is dropped.
            let Some((tc, tf)) = destub(metadata, (tc, tf), options) else {
                continue;
            };
            if ignored_contract(options, tc) {
                continue;
            }
            let target_id = overload_node_key(tc, &tf);
            if target_id == current.node_id {
                continue;
            }
            // A plain state-variable receiver (`debtToken.mint`) is typed by the deploy too,
            // and already has its arrow from the call-site pass; don't draw it twice.
            if origin.is_none()
                && edges
                    .iter()
                    .any(|edge| edge.from == current.node_id && edge.to == target_id)
            {
                continue;
            }
            let line_in_slice = slice
                .iter()
                .position(|l| line_has_call(l, method))
                .map(|i| i + 1)
                .unwrap_or(1);
            edges.push(GraphEdge {
                from: current.node_id.clone(),
                to: target_id.clone(),
                line_in_slice: line_in_slice + doc_shift,
                column: slice
                    .get(line_in_slice - 1)
                    .and_then(|l| l.find(method.as_str()))
                    .unwrap_or(0),
                symbol: method.clone(),
            });
            if leads_to_write(
                metadata,
                tc,
                &tf,
                &write_options,
                &write_definer_map,
                &mut write_memo,
                &mut HashSet::new(),
            ) {
                caller_write_calls.push((
                    function.line + line_in_slice - 1,
                    method.clone(),
                    target_id.clone(),
                ));
            }
            if drawn.insert(target_id.clone(), target_id.clone()).is_some() {
                continue;
            }
            let mut child = make_node(
                target_id.clone(),
                display_label(tc, &tf),
                tc,
                &tf,
                current.depth + 1,
                doc_lines_above(options, &tc.file_path, tf.line),
            );
            child.leads_to_write = leads_to_write(
                metadata,
                tc,
                &tf,
                &write_options,
                &write_definer_map,
                &mut write_memo,
                &mut HashSet::new(),
            );
            nodes.push(child);
            children.push(Pending {
                node_id: target_id,
                contract: tc.name.clone(),
                file: tc.file_path.clone(),
                function: tf.name.clone(),
                line: tf.line,
                depth: current.depth + 1,
            });
        }

        // Calls to external contracts with no in-scope source: flag the lines that
        // reach a non-view method (a `view`/`pure` one is compiler-guaranteed not to
        // mutate, so it is never a state-change risk). Located on the caller's node.
        let mut external_lines: Vec<usize> = std::mem::take(&mut lib_boundary_lines);
        for uec in &function.unknown_external_calls {
            let read_only = find_function(
                metadata,
                &uec.inferred_type,
                &contract.file_path,
                &uec.method,
                None,
            )
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
            if let Some(pos) = boundary_line_index(&slice, &uec.receiver, &uec.method) {
                // The scan finds implementers by inheritance only, so a contract that
                // matches the interface without declaring `is <interface>` leaves the call
                // listed here even though the call-site pass above drew its arrow into
                // in-scope code. That call doesn't leave the audited code: no amber.
                let drawn_in_scope = edges.iter().any(|e| {
                    e.from == current.node_id
                        && e.symbol == uec.method
                        && e.line_in_slice == pos + 1 + doc_shift
                });
                if drawn_in_scope {
                    continue;
                }
                external_lines.push(function.line + pos);
            }
        }
        if !caller_write_calls.is_empty() {
            caller_write_calls.sort_unstable();
            caller_write_calls.dedup();
            if let Some(node) = nodes.iter_mut().find(|n| n.id == current.node_id) {
                node.write_call_lines = caller_write_calls;
            }
        }
        if !caller_external_calls.is_empty() {
            caller_external_calls.sort_unstable();
            caller_external_calls.dedup();
            if let Some(node) = nodes.iter_mut().find(|n| n.id == current.node_id) {
                node.external_call_sites = caller_external_calls;
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

    // De-sharing (duplicating a shared function per caller) is decided AFTER render,
    // in `deploy_one`, where a preliminary layout tells which shared nodes actually
    // sit far from a caller and would cross — see `duplicate_crossing_shared`. A
    // blanket duplication here would repeat functions whose callers are adjacent
    // (no crossing) for nothing.
    for ((type_name, method), (candidates, locations)) in &left_out_reads {
        println!(
            "  {} {}.{}() is a read with {} implementations ({}), left out at {} — {} draws it",
            "note:".yellow(),
            type_name,
            method,
            candidates.len(),
            candidates.join(", "),
            locations.join(", "),
            format!("bat-cli resolve {type_name} <CONTRACT>").green()
        );
    }

    if !skipped_by_ignore.is_empty() {
        let mut listed: Vec<(String, usize)> = skipped_by_ignore.into_iter().collect();
        listed.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        println!(
            "  {} left out {}",
            "note:".yellow(),
            listed
                .iter()
                .map(|(name, count)| format!("{name} ({count} call site(s))"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    Ok((nodes, edges, unresolved))
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
                // No caller file survives into this frontier, so an ambiguous name keeps
                // the old first-match behaviour here.
                if let Some((_, f)) = find_function(metadata, &contract, "", &u.method, None) {
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
    doc_lines: usize,
) -> GraphNode {
    GraphNode {
        kind: NodeKind::Screenshot,
        id,
        label,
        file_path: contract.file_path.clone(),
        // The screenshot starts at the NatSpec when `--with-documentation` asked for it,
        // so the render key, the line gutter and every absolute-line lookup all agree on
        // where the image begins.
        start_line: function.line - doc_lines,
        end_line: function_end(function, contract),
        depth,
        // Rendered at the reference font (so one render serves every depth); the
        // depth's smaller look comes from `scale`, not a separate render.
        font_size: REFERENCE_FONT,
        scale: scale_for_depth(depth),
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
        leads_to_write: false,
        write_call_lines: Vec::new(),
        external_call_sites: Vec::new(),
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
/// Which line of a function's source carries the call to `method` on `receiver`.
///
/// Not simply the first line mentioning the method: a function whose own name matches its
/// callee's — `borrow` calling `MORPHO.borrow` — matched its own signature, and the band
/// landed on the header instead of on the call. So: prefer a line that also carries the
/// receiver, and never accept the declaration line itself.
fn boundary_line_index(slice: &[String], receiver: &str, method: &str) -> Option<usize> {
    slice
        .iter()
        .position(|line| {
            line_has_call(line, method) && !receiver.is_empty() && line.contains(receiver)
        })
        .or_else(|| {
            slice
                .iter()
                .enumerate()
                .skip(1)
                .find(|(_, line)| line_has_call(line, method))
                .map(|(index, _)| index)
        })
}

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
        // `(` for an ordinary call, `{` for one carrying a value or gas block —
        // `target.call{value: v}(data)` is the low-level call that moves ether, and
        // missing it meant the band never landed on the line that moves it.
        let is_call = cursor < bytes.len() && (bytes[cursor] == b'(' || bytes[cursor] == b'{');
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
    doc_lines: usize,
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
        start_line: definition.line - doc_lines,
        end_line,
        depth,
        font_size: REFERENCE_FONT,
        scale: scale_for_depth(depth),
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
        leads_to_write: false,
        write_call_lines: Vec::new(),
        external_call_sites: Vec::new(),
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
///
/// `from_file` is the file doing the naming: a contract name is resolved through ITS
/// imports (see `EvmBatMetadata::contract_in_scope`), and each base contract through the
/// imports of the contract that inherits it. Pass `""` when there is no such file.
fn find_function<'a>(
    metadata: &'a EvmBatMetadata,
    contract_name: &str,
    from_file: &str,
    function_name: &str,
    arg_count: Option<usize>,
) -> Option<(&'a ContractMetadata, FunctionMetadata)> {
    let contract = metadata.contract_in_scope(contract_name, from_file)?;
    let overloads: Vec<&FunctionMetadata> =
        contract.functions.iter().filter(|f| f.name == function_name).collect();
    if !overloads.is_empty() {
        // With several same-named overloads, pick the one whose parameter count
        // matches the call site; otherwise (or when the arity is unknown) keep the
        // first, which preserves the old single-definition behaviour exactly.
        let chosen = match arg_count {
            Some(n) if overloads.len() > 1 => overloads
                .iter()
                .find(|f| f.params.len() == n)
                .copied()
                .unwrap_or(overloads[0]),
            _ => overloads[0],
        };
        return Some((contract, chosen.clone()));
    }
    for base in &contract.base_contracts {
        if let Some(found) =
            find_function(metadata, base, &contract.file_path, function_name, arg_count)
        {
            return Some(found);
        }
    }
    None
}

/// Resolve to the SPECIFIC overload defined at `line` (walking base contracts),
/// so the DFS re-reads the same overload an edge was built for — not just the
/// first one sharing the name.
fn find_function_at<'a>(
    metadata: &'a EvmBatMetadata,
    contract_name: &str,
    from_file: &str,
    function_name: &str,
    line: usize,
) -> Option<(&'a ContractMetadata, FunctionMetadata)> {
    let contract = metadata.contract_in_scope(contract_name, from_file)?;
    if let Some(function) = contract
        .functions
        .iter()
        .find(|f| f.name == function_name && f.line == line)
    {
        return Some((contract, function.clone()));
    }
    for base in &contract.base_contracts {
        if let Some(found) =
            find_function_at(metadata, base, &contract.file_path, function_name, line)
        {
            return Some(found);
        }
    }
    None
}

/// Node id for a function, distinguishing overloads: when a contract defines the
/// same name several times, the id carries the definition line so each overload is
/// its own node (otherwise both collapse and a wrapper→overload call looks like a
/// self-call and is pruned). A single-definition function keeps the plain id, so
/// non-overloaded graphs are byte-identical to before.
/// Where an arrow meets a line of code: the point it lands ON, and the point outside the
/// box it converges at.
///
/// Every arrow in every diagram enters a screenshot the same way, so the arithmetic lives
/// here once: the head sits a couple of characters past the end of the line's text, the
/// convergence sits on the box's border level with it, and a line that runs the full width
/// pushes that convergence out past the border so the stub between them still has length
/// (a zero-length stub draws no arrow head at all). `deploy` uses it for a call site and a
/// type frame for the field that names a type — the second was a copy of this that had
/// already drifted on the size of the gap.
pub(crate) struct LineAnchor {
    /// Where the head lands: past the end of the text, inside the box when it fits.
    pub token_x: f64,
    pub token_y: f64,
    /// Where the arrows converge: on the border, or outside it for a full-width line.
    pub edge_x: f64,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn line_anchor(
    // Box on the board: centre, width, height.
    placed: (f64, f64, f64, f64),
    png_width: u32,
    png_height: u32,
    font_size: usize,
    rendered_lines: &[String],
    line_offset: usize,
    line_index: usize,
    exit_right: bool,
) -> LineAnchor {
    let (x, y, width, height) = placed;
    let line_text = rendered_lines.get(line_index).cloned().unwrap_or_default();
    let text_width = |text: &str| {
        silicon::line_end_x(Some(font_size), true, rendered_lines.len(), line_offset, text) as f64
    };
    let y_fraction = silicon::line_geometry(Some(font_size)).line_center_fraction(line_index, png_height);
    let token_frac = if png_width > 0 {
        let png = png_width as f64;
        if exit_right {
            let gap = (text_width("a") - text_width("")) * ANCHOR_GAP_CHARS;
            ((text_width(&line_text) + gap) / png).min(1.0)
        } else {
            (text_width("") / png).max(0.0)
        }
    } else if exit_right {
        1.0
    } else {
        0.0
    };
    let frame_edge = if exit_right { x + width / 2.0 } else { x - width / 2.0 };
    let raw_token_x = x - width / 2.0 + width * token_frac;
    let min_stub = 60.0_f64;
    let edge_gap = 200.0_f64;
    let reaches_edge = exit_right && raw_token_x > frame_edge - min_stub * 2.0;
    let edge_x = if reaches_edge { frame_edge + edge_gap } else { frame_edge };
    let token_x = if exit_right {
        raw_token_x.min(edge_x - min_stub)
    } else {
        raw_token_x.max(edge_x + min_stub)
    };
    LineAnchor {
        token_x,
        token_y: y - height / 2.0 + height * y_fraction,
        edge_x,
    }
}

/// Give every callee a colour no callee it is compared with already has.
///
/// Greedy colouring, busiest node first: a function whose arrows are compared with many
/// others needs a free colour more than one compared with two, and greedy is only as good
/// as the order it walks in. `palette_for` hands back the order to try colours in, which
/// is how each gutter gets its own shuffle.
///
/// When a callee has no free colour left — more mutually-compared functions in one frame
/// than the palette holds — it repeats one and is returned in the second set, for the
/// caller to draw dashed. A repeated colour that is also a repeated line style would be
/// two arrows a reader cannot tell apart, which is the one thing colour is for.
fn color_callees<'a>(
    conflicts: &HashMap<&'a str, HashSet<&'a str>>,
    mut palette_for: impl FnMut(&'a str) -> Vec<usize>,
) -> HashMap<&'a str, (usize, ConnectorStroke)> {
    let mut callees: Vec<&'a str> = conflicts.keys().copied().collect();
    callees.sort_by(|a, b| {
        let degree = |id: &&str| conflicts.get(*id).map(|set| set.len()).unwrap_or(0);
        degree(b).cmp(&degree(a)).then(a.cmp(b))
    });

    let mut chosen: HashMap<&'a str, (usize, ConnectorStroke)> = HashMap::new();
    for callee in callees {
        let used: HashSet<(usize, ConnectorStroke)> = conflicts
            .get(callee)
            .map(|rivals| rivals.iter().filter_map(|rival| chosen.get(rival).copied()).collect())
            .unwrap_or_default();
        let palette = palette_for(callee);
        // What a reader has to tell apart is the PAIR: a hue and a line. Running out of
        // hues used to hand every remaining callee the same colour and the same dashes, so
        // two arrows side by side became one — which is what `cross` and `deployPool` showed
        // in the same column. Dotted is the second register, and dashed stays reserved for
        // a cycle.
        let combinations = [ConnectorStroke::Solid, ConnectorStroke::Dotted]
            .into_iter()
            .flat_map(|stroke| palette.iter().copied().map(move |color| (color, stroke)));
        let free = combinations.clone().find(|pair| !used.contains(pair));
        chosen.insert(
            callee,
            free.or_else(|| combinations.into_iter().next())
                .unwrap_or((0, ConnectorStroke::Solid)),
        );
    }
    chosen
}

fn overload_node_key(contract: &ContractMetadata, function: &FunctionMetadata) -> String {
    match overload_signature(contract, function) {
        Some(signature) => format!("{}{signature}", node_key(&contract.name, &function.name)),
        None => node_key(&contract.name, &function.name),
    }
}

/// `(uint256,address)` when this contract declares the name more than once, else nothing.
///
/// Solidity requires overloads to differ in their parameter types, so the signature tells
/// them apart — and unlike the line number that did this job before, it is the thing
/// somebody reading the source would use to say WHICH `read` they mean. The node id and
/// the label a frame is titled with then differ only by `::` versus `.`, which is what
/// lets a pasted frame be re-paired by its title.
fn overload_signature(contract: &ContractMetadata, function: &FunctionMetadata) -> Option<String> {
    if contract.functions.iter().filter(|f| f.name == function.name).count() <= 1 {
        return None;
    }
    let types = function
        .params
        .iter()
        .map(|param| param.type_name.as_str())
        .collect::<Vec<_>>()
        .join(",");
    Some(format!("({types})"))
}

/// What a frame is called: `Contract.function`, plus the signature when the name alone
/// would not say which one.
fn display_label(contract: &ContractMetadata, function: &FunctionMetadata) -> String {
    match overload_signature(contract, function) {
        Some(signature) => format!("{}.{}{signature}", contract.name, function.name),
        None => format!("{}.{}", contract.name, function.name),
    }
}

/// The constructors that run before `contract`'s own, nearest first along each base.
///
/// A base without a constructor of its own still runs ITS bases' constructors, so the walk
/// descends through it. Each base name is resolved through the imports of the contract that
/// inherits it, so a vendored copy of the library that nobody imports is never picked.
fn base_constructors<'a>(
    metadata: &'a EvmBatMetadata,
    contract: &ContractMetadata,
) -> Vec<(&'a ContractMetadata, FunctionMetadata)> {
    let mut found: Vec<(&'a ContractMetadata, FunctionMetadata)> = Vec::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut pending: Vec<(String, String)> = contract
        .base_contracts
        .iter()
        .rev()
        .map(|base| (base.clone(), contract.file_path.clone()))
        .collect();
    while let Some((name, from_file)) = pending.pop() {
        let Some(base) = metadata.contract_in_scope(&name, &from_file) else {
            continue;
        };
        if !seen.insert((base.name.clone(), base.file_path.clone())) {
            continue;
        }
        if let Some(constructor) = base.functions.iter().find(|f| f.is_constructor) {
            found.push((base, constructor.clone()));
        } else {
            for grand in base.base_contracts.iter().rev() {
                pending.push((grand.clone(), base.file_path.clone()));
            }
        }
    }
    found
}

fn find_modifier<'a>(
    metadata: &'a EvmBatMetadata,
    contract_name: &str,
    from_file: &str,
    modifier_name: &str,
) -> Option<(&'a ContractMetadata, crate::batbelt::evm::types::EvmModifierDef)> {
    let contract = metadata.contract_in_scope(contract_name, from_file)?;
    if let Some(definition) = contract.modifiers.iter().find(|m| m.name == modifier_name) {
        return Some((contract, definition.clone()));
    }
    for base in &contract.base_contracts {
        if let Some(found) = find_modifier(metadata, base, &contract.file_path, modifier_name) {
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
/// The call lines that actually get a mark on this frame.
///
/// One state change must produce ONE mark. A chain like
/// `_increaseDebt → DebtToken.mint → ERC20._mint → ERC20._update` would otherwise paint
/// every hop for the single write in `_update`, and counting the red marks on a frame
/// would tell the reader nothing.
///
/// So a call line keeps its mark only when the callee is NOT a screenshot on this frame:
/// then this line is the deepest the frame gets to the write, and the last place able to
/// show it. When the callee IS drawn, it carries the mark instead — its own assignment, or
/// its own outgoing call — which is nearer to where the change really happens. A callee
/// cut out to another frame (a link card) counts as absent, since the mark belongs to the
/// frame that draws it.
fn surviving_write_calls<'a>(
    node: &'a GraphNode,
    drawn_screens: &HashSet<&str>,
) -> impl Iterator<Item = &'a (usize, String, String)> {
    let drawn: HashSet<String> = drawn_screens.iter().map(|id| id.to_string()).collect();
    node.write_call_lines
        .iter()
        .filter(move |(_, _, callee)| !drawn.contains(callee))
}

/// The same rule for the amber marks: a call reaching a boundary keeps its band only when
/// the callee is not drawn here, so the frame nearest the boundary is the one that shows it.
fn surviving_external_calls<'a>(
    node: &'a GraphNode,
    drawn_screens: &HashSet<&str>,
) -> impl Iterator<Item = &'a (usize, String, String)> {
    let drawn: HashSet<String> = drawn_screens.iter().map(|id| id.to_string()).collect();
    node.external_call_sites
        .iter()
        .filter(move |(_, _, callee)| !drawn.contains(callee))
}

/// Node ids drawn as source on this frame, which is what decides who owns a mark.
fn drawn_screen_ids(nodes: &[GraphNode]) -> HashSet<&str> {
    nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Screenshot)
        .map(|node| node.id.as_str())
        .collect()
}

/// Does this function reach a storage write, directly or through anything it calls?
///
/// A node is drawn red only when it holds the assignment itself, and that hides real state
/// changes: `PositionManager._increaseDebt` calls `debtToken.mint`, which calls
/// `ERC20._mint`, which in OpenZeppelin v5 only validates and delegates to `_update` —
/// the single function in the chain that assigns anything. Every hop in between looked
/// inert on the board.
///
/// The walk is over the METADATA call graph, not the drawn one, and it crosses into
/// `lib/` deliberately: whether a call changes state is a fact about the code, not about
/// how much of it this deploy chose to draw. That is what keeps the mark visible when the
/// chain is cut short by framing, `--max-depth`, or leaving `--include-external` off.
///
/// `function_dependencies.callees` holds the same call strings `resolve_call` consumes
/// (`_increaseDebt`, `debtToken.mint`, `PropMath._computeNominalCR`), so resolution here
/// is exactly the resolution used to draw the graph.
/// Does this function reach a boundary the diagram cannot follow past?
///
/// The amber counterpart of `leads_to_write`, and it exists for the same reason: the frame
/// that draws the boundary should carry the mark, but a frame that stops short of it has
/// to say so on the call, or a chain that moves value ends in silence. `SafeERC20.
/// safeTransferFrom` is four hops from the `call` that actually moves the tokens.
fn leads_to_external(
    metadata: &EvmBatMetadata,
    contract: &ContractMetadata,
    function: &FunctionMetadata,
    options: &AutoDeployOptions,
    definer_map: &HashMap<String, Vec<String>>,
    memo: &mut HashMap<String, bool>,
    stack: &mut HashSet<String>,
) -> bool {
    if !function.unknown_external_calls.is_empty() {
        return true;
    }
    let id = function.metadata_id.clone();
    if let Some(&cached) = memo.get(&id) {
        return cached;
    }
    if !stack.insert(id.clone()) {
        return false;
    }
    let callees = metadata
        .function_dependencies
        .iter()
        .find(|dependency| dependency.function_metadata_id == id)
        .map(|dependency| dependency.callees.clone())
        .unwrap_or_default();
    let mut reaches = false;
    for callee in callees {
        // Resolve as the drawing does; and when that cannot decide — a bare name from a
        // `using X for Y` call, whose arity at the call site is one short of the library
        // function's — fall back to every contract that defines the name. This walk only
        // decides whether to MARK a line, so a wider net costs a mark, not a wrong box.
        let method = callee.split('.').next_back().unwrap_or(&callee);
        let mut reached: Vec<(&ContractMetadata, FunctionMetadata)> =
            match resolve_call(metadata, contract, &callee, None, options, definer_map) {
                // EVERY overload, not just the one an unknown arity picks first. An
                // overload commonly delegates to its longer sibling — `functionCall(a, b)`
                // calls `functionCall(a, b, msg)` — and by name those are the same callee,
                // so following only the first walked straight back into itself, hit the
                // cycle guard, and reported that a chain ending in a token transfer
                // reached nothing at all.
                Some((found_contract, _)) => found_contract
                    .functions
                    .iter()
                    .filter(|f| f.name == method)
                    .map(|f| (found_contract, f.clone()))
                    .collect(),
                None => definer_map
                    .get(callee.split('.').next_back().unwrap_or(&callee))
                    .map(|definers| {
                        definers
                            .iter()
                            .filter_map(|name| {
                                let target =
                                    metadata.contract_in_scope(name, &contract.file_path)?;
                                find_function(
                                    metadata,
                                    &target.name,
                                    &target.file_path,
                                    callee.split('.').next_back().unwrap_or(&callee),
                                    None,
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            };
        for (next_contract, next_function) in reached.drain(..) {
            if leads_to_external(
                metadata,
                next_contract,
                &next_function,
                options,
                definer_map,
                memo,
                stack,
            ) {
                reaches = true;
                break;
            }
        }
        if reaches {
            break;
        }
    }
    stack.remove(&id);
    memo.insert(id, reaches);
    reaches
}

fn leads_to_write(
    metadata: &EvmBatMetadata,
    contract: &ContractMetadata,
    function: &FunctionMetadata,
    options: &AutoDeployOptions,
    definer_map: &HashMap<String, Vec<String>>,
    memo: &mut HashMap<String, bool>,
    stack: &mut HashSet<String>,
) -> bool {
    if !function.storage_writes.is_empty() {
        return true;
    }
    let id = function.metadata_id.clone();
    if let Some(&cached) = memo.get(&id) {
        return cached;
    }
    // Recursion is normal in a call graph; a cycle contributes nothing on its own.
    if !stack.insert(id.clone()) {
        return false;
    }

    let callees = metadata
        .function_dependencies
        .iter()
        .find(|dependency| dependency.function_metadata_id == id)
        .map(|dependency| dependency.callees.clone())
        .unwrap_or_default();

    let mut reaches = false;
    for callee in callees {
        if let Some((target_contract, target_function)) =
            resolve_call(metadata, contract, &callee, None, options, definer_map)
        {
            if leads_to_write(
                metadata,
                target_contract,
                &target_function,
                options,
                definer_map,
                memo,
                stack,
            ) {
                reaches = true;
                break;
            }
        }
    }

    // Calls the call-string resolver cannot reach: those pinned by type in the scan
    // (`$.priceFeed.pegOk`) and interface calls the AI resolved.
    if !reaches {
        let followed = function
            .resolved_calls
            .iter()
            .map(|typed| (typed.contract.clone(), typed.method.clone()))
            .chain(function.unresolved_calls.iter().filter_map(|u| {
                metadata
                    .resolutions
                    .get(&u.inferred_type)
                    .map(|concrete| (concrete.clone(), u.method.clone()))
            }))
            .collect::<Vec<_>>();
        for (concrete, method) in followed {
            if let Some((target_contract, target_function)) =
                find_function(metadata, &concrete, &contract.file_path, &method, None)
            {
                if leads_to_write(
                    metadata,
                    target_contract,
                    &target_function,
                    options,
                    definer_map,
                    memo,
                    stack,
                ) {
                    reaches = true;
                    break;
                }
            }
        }
    }

    stack.remove(&id);
    memo.insert(id, reaches);
    reaches
}

fn resolve_call<'a>(
    metadata: &'a EvmBatMetadata,
    caller_contract: &ContractMetadata,
    call_name: &str,
    arg_count: Option<usize>,
    options: &AutoDeployOptions,
    definer_map: &HashMap<String, Vec<String>>,
) -> Option<(&'a ContractMetadata, FunctionMetadata)> {
    let keep = |_contract: &ContractMetadata| true;

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
        // `IFace(addr).method()`, rendered `IFace()`: a cast of a runtime address.
        Some(target) if target.ends_with("()") => {
            return resolve_cast(
                metadata,
                caller_contract,
                target.trim_end_matches("()"),
                method,
                arg_count,
                options,
            );
        }
        Some(target) => {
            if metadata.contract_in_scope(target, &caller_contract.file_path).is_some() {
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
        let Some(contract) = metadata.contract_in_scope(&candidate, &caller_contract.file_path)
        else {
            continue;
        };
        if contract.contract_type == EvmContractType::Interface {
            // An interface has no body worth screenshotting; jump to the impl.
            for implementation in implementations_of(metadata, &contract.name) {
                if let Some(target) =
                    metadata.contract_in_scope(&implementation, &caller_contract.file_path)
                {
                    if !keep(target) {
                        continue;
                    }
                    if let Some(found) =
                        find_function(metadata, &target.name, &target.file_path, method, arg_count)
                    {
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
        if let Some(found) =
            find_function(metadata, &contract.name, &contract.file_path, method, arg_count)
        {
            if let Some(resolved) = destub(metadata, found, options) {
                return Some(resolved);
            }
        }
    }

    // Libraries bound with `using X for Y;`. A bare method this contract and its bases do
    // not define may still be one of theirs — that is what the directive says, and it is
    // the only thing in the source that says it. The receiver becomes the library
    // function's FIRST parameter, so `address(token).functionCall(data, msg)` reads as two
    // arguments at the call site and three in `Address.functionCall`: the arity has to be
    // shifted, or every one of these resolves to nothing.
    for library in &caller_contract.using_libraries {
        let Some(contract) = metadata.contract_in_scope(library, &caller_contract.file_path) else {
            continue;
        };
        for arity in [arg_count.map(|count| count + 1), arg_count, None] {
            if let Some(found) =
                find_function(metadata, &contract.name, &contract.file_path, method, arity)
            {
                if let Some(resolved) = destub(metadata, found, options) {
                    return Some(resolved);
                }
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
                return find_function(
                    metadata,
                    &definers[0],
                    &caller_contract.file_path,
                    method,
                    arg_count,
                );
            }
        }
    }
    None
}

/// Resolve `Type(addr).method()`.
///
/// Casting to a concrete contract names the code outright. Casting to an interface does
/// not: which implementation sits behind the address is decided when the system is
/// deployed, which no reading of the source can reveal. So an interface cast is followed
/// only when that question has an answer — a recorded `bat-cli resolve`, or a single
/// implementation defining the method. Several candidates return `None`; the graph
/// builder then asks (see `cast_implementations`) instead of drawing a guess.
fn resolve_cast<'a>(
    metadata: &'a EvmBatMetadata,
    caller_contract: &ContractMetadata,
    type_name: &str,
    method: &str,
    arg_count: Option<usize>,
    options: &AutoDeployOptions,
) -> Option<(&'a ContractMetadata, FunctionMetadata)> {
    let keep = |_contract: &ContractMetadata| true;
    let typed = metadata.contract_in_scope(type_name, &caller_contract.file_path)?;

    if typed.contract_type != EvmContractType::Interface {
        if !keep(typed) {
            return None;
        }
        let found = find_function(metadata, &typed.name, &typed.file_path, method, arg_count)?;
        return destub(metadata, found, options);
    }

    if let Some(concrete) = metadata.resolutions.get(type_name) {
        let target = metadata.contract_in_scope(concrete, &caller_contract.file_path)?;
        if !keep(target) {
            return None;
        }
        let found = find_function(metadata, &target.name, &target.file_path, method, arg_count)?;
        return destub(metadata, found, options);
    }

    let mut implementations = cast_implementations(metadata, caller_contract, type_name, method, arg_count);
    implementations.retain(|(contract, _)| keep(contract));
    if implementations.len() == 1 {
        return implementations.pop();
    }
    None
}

/// Every concrete IN-SCOPE contract that implements interface `type_name` and defines
/// `method` with a body, one per name. None means the target is an external contract.
///
/// Same-named copies collapse to the one reachable from the caller's imports, so three
/// vendored `UpgradeableBeacon`s count as one candidate, not three.
fn cast_implementations<'a>(
    metadata: &'a EvmBatMetadata,
    caller_contract: &ContractMetadata,
    type_name: &str,
    method: &str,
    arg_count: Option<usize>,
) -> Vec<(&'a ContractMetadata, FunctionMetadata)> {
    let mut names: Vec<String> = implementations_of(metadata, type_name);
    names.sort();
    names.dedup();
    let mut found = Vec::new();
    for name in names {
        if name == type_name {
            continue;
        }
        let Some(contract) = metadata.contract_in_scope(&name, &caller_contract.file_path) else {
            continue;
        };
        if contract.contract_type == EvmContractType::Interface {
            continue;
        }
        // The same rule the scan applies (`compute_unresolved_calls`): a generic library
        // implementation is not what a runtime address points at. `IERC20(token)` is some
        // deployed token, never OpenZeppelin's `ERC20` template, and offering the template
        // as a candidate invites a global `bat-cli resolve IERC20 ERC20` that would bind
        // every IERC20 cast in the project to the wrong code.
        if contract.external {
            continue;
        }
        if let Some((defining, function)) =
            find_function(metadata, &contract.name, &contract.file_path, method, arg_count)
        {
            if !function.is_stub {
                found.push((defining, function));
            }
        }
    }
    found
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
    let keep = |_c: &ContractMetadata| true;
    let mut overrides: Vec<(&'a ContractMetadata, FunctionMetadata)> = Vec::new();
    for impl_name in implementations_of(metadata, &contract.name) {
        if impl_name == contract.name {
            continue;
        }
        if let Some((tc, tf)) = find_function(
            metadata,
            &impl_name,
            &contract.file_path,
            &function.name,
            Some(function.params.len()),
        )
        {
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

/// First line of the NatSpec block written directly above `decl_line`, or `decl_line`
/// itself when there is none.
///
/// "Directly above" is deliberate: a comment separated from the declaration by a blank
/// line belongs to whatever came before it as often as not, and pulling it in would put
/// another function's documentation on this one's screenshot. Both NatSpec forms count —
/// a run of `///` lines, or one `/** … */` block — and nothing else does, so an ordinary
/// `//` note is left out.
pub(crate) fn natspec_start(file_path: &str, decl_line: usize) -> usize {
    let content = std::fs::read_to_string(file_path).unwrap_or_default();
    let lines: Vec<&str> = content.lines().collect();
    // `decl_line` is 1-based, so the line above it is at index `decl_line - 2`.
    if decl_line < 2 || decl_line > lines.len() {
        return decl_line;
    }
    let mut index = decl_line - 2;
    let above = lines[index].trim();

    if above.ends_with("*/") {
        // Walk up to the line that opens the block. A one-line `/** … */` opens and
        // closes on the same line, which this handles too.
        loop {
            let line = lines[index].trim();
            if line.starts_with("/**") || line.starts_with("/*") {
                return index + 1;
            }
            if index == 0 {
                return decl_line; // unterminated — take nothing rather than guess
            }
            index -= 1;
        }
    }

    if !above.starts_with("///") {
        return decl_line;
    }
    // Consume the whole run of `///` lines.
    while index > 0 && lines[index - 1].trim().starts_with("///") {
        index -= 1;
    }
    index + 1
}

/// How many lines `--with-documentation` prepends to this declaration's screenshot.
///
/// Every rendered line above the declaration shifts the anchors computed from the body,
/// so this same number is added to each edge's `line_in_slice`.
fn doc_lines_above(options: &AutoDeployOptions, file_path: &str, decl_line: usize) -> usize {
    if !options.with_documentation {
        return 0;
    }
    decl_line.saturating_sub(natspec_start(file_path, decl_line))
}

pub(crate) fn read_slice(file_path: &str, start_line: usize, end_line: usize) -> Vec<String> {
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
///
/// `reuse` maps a node id to its already-uploaded image and measured size: those
/// nodes are NOT re-rendered (their size is taken as given), which is what makes
/// `--refresh-links` cheap. An empty map is a normal full render.
fn render_and_measure(
    nodes: &mut [GraphNode],
    owner: &str,
    reuse: &HashMap<String, (String, u32, u32)>,
) -> Result<()> {
    // Screenshots are scratch: they are deleted once uploaded, so the directory
    // is often not there. Create it rather than treating its absence as an
    // error the user has to fix.
    BatFolder::Figures
        .create_folder()
        .change_context(EvmMiroError)?;
    let destination = BatFolder::Figures
        .get_path(true)
        .change_context(EvmMiroError)?;

    // Render DEDUP: every node now renders at the reference font, so two nodes with
    // the same (file, line range) produce a byte-identical image — a function used
    // (or duplicated) N times used to render N identical PNGs, the dominant cost on
    // a big diagram. Render each DISTINCT (file, start, end) once and share it; the
    // per-depth smaller look is applied later via `scale`, not another render.
    type RenderKey = (String, usize, usize);
    let key_of = |node: &GraphNode| -> Option<RenderKey> {
        (!node.file_path.is_empty() && !reuse.contains_key(&node.id))
            .then(|| (node.file_path.clone(), node.start_line, node.end_line))
    };
    let mut distinct: Vec<RenderKey> = Vec::new();
    let mut seen: HashSet<RenderKey> = HashSet::new();
    for node in nodes.iter() {
        if let Some(key) = key_of(node) {
            if seen.insert(key.clone()) {
                distinct.push(key);
            }
        }
    }

    struct Rendered {
        png_path: String,
        width: u32,
        height: u32,
        line_offset: usize,
        lines: Vec<String>,
    }

    let bar = phase_bar("rendering screenshots", distinct.len());
    // CPU-bound and independent per DISTINCT function — fans out across cores.
    let results: std::result::Result<Vec<(RenderKey, Rendered)>, String> = distinct
        .par_iter()
        .map(|key| {
            let (file_path, start, end) = key;
            let code = read_slice(file_path, *start, *end);
            let pretty_path = crate::batbelt::path::prettify_source_code_path(file_path)
                .unwrap_or_else(|_| file_path.clone());
            let mut lines = vec![format!("// {pretty_path}"), String::new()];
            lines.extend(code.iter().cloned());
            let line_offset = start.saturating_sub(PATH_HEADER_LINES);
            // One file per DISTINCT function, named ONLY by (file, lines) — no
            // deployment prefix — so every frame in the same run (a whole --redeploy
            // cluster) SHARES it: a function that appears in several frames is rendered
            // once for the entire run, not once per frame. Cleanup is deferred to the
            // end of the run so the shared files survive between frames.
            let file_name = format!(
                "fn_{}_{}_{}.js",
                file_path.replace([':', '.', '/'], "_"),
                start,
                end
            );
            let png_path = format!("{destination}/{file_name}.png");
            // Cache hit: an earlier frame this run already rendered this function. The
            // file has to be WHOLE to count as one — a run killed mid-render leaves a
            // truncated PNG behind, and since the cache is keyed by name alone every
            // later run inherits it and dies measuring it. So the render goes to a
            // private name and is moved into place in one step: a reader either sees the
            // finished file or no file.
            if !std::path::Path::new(&png_path).exists() {
                let traced = traced_names(&lines);
                let partial = partial_render_name(&file_name, std::process::id());
                silicon::create_figure_tracing(
                    &lines.join("\n"),
                    &destination,
                    &partial,
                    line_offset,
                    Some(REFERENCE_FONT),
                    true,
                    &traced,
                );
                let partial_path = format!("{destination}/{partial}.png");
                std::fs::rename(&partial_path, &png_path)
                    .map_err(|e| format!("cannot store {png_path}: {e}"))?;
            }
            let (width, height) = image::image_dimensions(&png_path)
                .map_err(|e| format!("cannot measure {png_path}: {e}"))?;
            if width > 8192 || height > 8192 {
                log::warn!("{png_path} renders to {width}x{height}, above Miro's 8192 px limit");
            }
            bar.inc(1);
            Ok((
                key.clone(),
                Rendered { png_path, width, height, line_offset, lines },
            ))
        })
        .collect();
    let rendered: HashMap<RenderKey, Rendered> = results
        .map_err(|message| Report::new(EvmMiroError).attach_printable(message))?
        .into_iter()
        .collect();
    bar.finish_and_clear();

    // Fan the shared renders (and reused sizes) back onto every node.
    for node in nodes.iter_mut() {
        if let Some((_, width, height)) = reuse.get(&node.id) {
            node.png_width = *width;
            node.png_height = *height;
            continue;
        }
        let Some(key) = key_of(node) else { continue };
        if let Some(r) = rendered.get(&key) {
            node.png_path = r.png_path.clone();
            node.png_width = r.width;
            node.png_height = r.height;
            node.line_offset = r.line_offset;
            node.rendered_lines = r.lines.clone();
        }
    }
    println!(
        "    {} {} screenshots ({} distinct rendered)",
        "✓".green(),
        nodes.len(),
        distinct.len()
    );
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

/// Set this and a run leaves its rendered screenshots in the figures directory instead of
/// wiping them. A dry run otherwise draws every screenshot of a cluster and deletes them a
/// second later, which makes the one thing it is best placed to show — what the code will
/// actually LOOK like — impossible to inspect.
const KEEP_FIGURES_ENV: &str = "BAT_CLI_KEEP_FIGURES";

fn cleanup(nodes: &[GraphNode]) {
    if std::env::var_os(KEEP_FIGURES_ENV).is_some() {
        return;
    }
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
        "  {:<38} {:>5} {:>9} {:>9} {:>7} {:>7}  {:<11}  {}",
        "node", "layer", "x", "y", "w", "h", "png", "state"
    );
    let drawn_screens = drawn_screen_ids(nodes);
    let mut placed: Vec<_> = layout.nodes.iter().collect();
    placed.sort_by_key(|node| (node.layer, node.y as i64));
    for node in placed {
        let source = nodes.iter().find(|n| n.id == node.id);
        println!(
            "  {:<38} {:>5} {:>9.0} {:>9.0} {:>7.0} {:>7.0}  {:<11}  {}",
            truncate(&source.map(|n| n.label.clone()).unwrap_or_default(), 38),
            node.layer,
            node.x,
            node.y,
            node.width,
            node.height,
            source
                .map(|n| format!("{}x{}", n.png_width, n.png_height))
                .unwrap_or_default(),
            // What this node actually gets drawn with: `write` holds the assignment,
            // `→write` carries the mark for a change that happens past the frame's edge.
            // A node that only passes the call along shows nothing — the callee owns it.
            source
                .map(|n| {
                    if n.writes_storage {
                        "write".to_string()
                    } else if surviving_write_calls(n, &drawn_screens).next().is_some() {
                        "→write".to_string()
                    } else {
                        String::new()
                    }
                })
                .unwrap_or_default()
        );
    }

    let marked_lines: Vec<String> = nodes
        .iter()
        .flat_map(|node| {
            surviving_write_calls(node, &drawn_screens)
                .map(move |(line, symbol, _)| format!("{} L{line} → {symbol}()", node.label))
        })
        .collect();
    if !marked_lines.is_empty() {
        println!("  {} call line(s) reaching a state change:", marked_lines.len());
        for line in marked_lines {
            println!("    {line}");
        }
    }
    let external_lines: Vec<String> = nodes
        .iter()
        .flat_map(|node| node.external_call_lines.iter().map(move |line| format!("{} L{line}", node.label)))
        .collect();
    if !external_lines.is_empty() {
        println!("  {} external boundary line(s) (amber):", external_lines.len());
        for line in external_lines {
            println!("    {line}");
        }
    }

    // The span is how many columns the arrow flies over, computed from the placed
    // NODES rather than their labels: a copied helper shares its label with every
    // other copy, so counting by name reports the shortest arrow that could have been
    // drawn instead of the one that will be.
    let mut spans: Vec<usize> = Vec::new();
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
        let span = match (layout.node(&edge.from), layout.node(&edge.to)) {
            (Some(from), Some(to)) => to.layer.saturating_sub(from.layer),
            _ => 0,
        };
        spans.push(span);
        println!(
            "    {:<34} L{:<5} → {:<34} start ({:.2}%, {:.2}%){}",
            truncate(&caller.label, 34),
            caller.start_line + edge.line_in_slice - 1,
            truncate(&callee_label, 34),
            anchor.x_fraction * 100.0,
            anchor.y_fraction * 100.0,
            if span > 1 { format!("  spans {span} columns") } else { String::new() }
        );
    }
    let crossing = spans.iter().filter(|span| **span > 1).count();
    println!(
        "  {} of {} connector(s) fly over a column (worst {})",
        crossing,
        spans.len(),
        spans.iter().max().copied().unwrap_or(0)
    );
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

/// De-share ONLY the functions whose reuse tangles the diagram. A shared node
/// sitting several columns back from a caller draws a long arrow that crosses the
/// screenshots between them (Miro routes connectors itself, so the layout can't
/// bend around them); that far caller gets a local copy of the node's private
/// closure instead — a repeated screenshot, which the auditor prefers to a
/// crossing. A caller in the adjacent column keeps sharing the one node, so nothing
/// is repeated for free. Runs after render, so copies inherit the image.
///
/// Each round lays the graph out, finds the shared node with the farthest-back
/// caller, and copies its closure for every caller ≥ `CROSS_LAYERS` columns back —
/// keeping the NEAREST caller on the original so it is never orphaned (with one
/// caller left, layering places the node adjacent, so it no longer crosses).
/// Re-lays-out each round because a copy changes the columns; bounded by closure
/// size and a box budget.
/// Replace the CROSSING CALL — not the callee — with a card, when the callee is too
/// big to copy next to its far caller.
///
/// Framing cuts for SPACE: a branch leaves the frame when the frame is too big to
/// read. That left a whole class of mess untouched — a helper called from columns 1
/// and 3 is pinned to the far right by the longest-path layering, so its arrow from
/// column 1 flies over everything in between, and no amount of space made that
/// arrow shorter. The only two ways to shorten it are a copy next to each caller or
/// a card next to each caller, and a card is what a callee too big to copy gets.
///
/// It cuts the offending EDGE, not the node: the caller sitting next to the callee
/// keeps reading it as a screenshot, and only the caller that was flying an arrow
/// over two columns gets a card instead. Replacing the node would take the drawing
/// away from the near caller too, to fix a problem that caller never had.
///
/// Returns how many calls were cut, so the caller can say so.
fn cut_crossing_shared(nodes: &mut Vec<GraphNode>, edges: &mut Vec<GraphEdge>, root_id: &str) -> usize {
    const CROSS_LAYERS: usize = 2;
    let mut done = 0usize;
    loop {
        let layout_nodes: Vec<LayoutNode> = nodes
            .iter()
            .map(|node| LayoutNode {
                id: node.id.clone(),
                width: node.board_width(),
                height: node.board_height(),
            })
            .collect();
        let anchors = compute_anchors(nodes, edges);
        let layout_edges: Vec<LayoutEdge> = edges
            .iter()
            .zip(anchors.iter())
            .map(|(edge, anchor)| LayoutEdge {
                from: edge.from.clone(),
                to: edge.to.clone(),
                from_line_fraction: anchor.y_fraction,
            })
            .collect();
        let layout = layout_graph(root_id, &layout_nodes, &layout_edges, LayoutConfig::default());
        let layer_of: HashMap<&str, usize> =
            layout.nodes.iter().map(|p| (p.id.as_str(), p.layer)).collect();

        let mut callers: HashMap<String, Vec<String>> = HashMap::new();
        let mut out: HashMap<String, Vec<String>> = HashMap::new();
        for edge in edges.iter() {
            callers.entry(edge.to.clone()).or_default().push(edge.from.clone());
            out.entry(edge.from.clone()).or_default().push(edge.to.clone());
        }
        let shared: HashSet<String> = callers
            .iter()
            .filter(|(_, cs)| cs.len() >= 2)
            .map(|(id, _)| id.clone())
            .collect();

        // The biggest crosser first: it is the one whose frame is most worth having,
        // and cutting it often takes several other crossers with it.
        let mut victim: Option<(usize, String)> = None;
        for id in &shared {
            let is_screenshot = nodes
                .iter()
                .any(|node| node.id == *id && matches!(node.kind, NodeKind::Screenshot));
            if !is_screenshot || id == root_id {
                continue;
            }
            let worst = callers
                .get(id)
                .map(|cs| {
                    cs.iter()
                        .map(|c| match (layer_of.get(id.as_str()), layer_of.get(c.as_str())) {
                            (Some(&vl), Some(&cl)) => vl.saturating_sub(cl),
                            _ => 0,
                        })
                        .max()
                        .unwrap_or(0)
                })
                .unwrap_or(0);
            if worst < CROSS_LAYERS {
                continue;
            }
            // Two different costs, so two different thresholds. Copying a callee costs
            // its closure ONCE PER far caller, so a helper the size of a getter is still
            // expensive when nine callers need their own copy — that is how one contract
            // put 203 boxes on a board for 30 functions. Either cost alone is enough to
            // prefer a frame: one drawing, a card beside each caller.
            let clen = private_closure(id, &out, &shared).len();
            let far = callers
                .get(id)
                .map(|cs| {
                    let mut distinct: Vec<&String> = cs.iter().collect();
                    distinct.sort();
                    distinct.dedup();
                    distinct
                        .iter()
                        .filter(|c| match (layer_of.get(id.as_str()), layer_of.get(c.as_str())) {
                            (Some(&vl), Some(&cl)) => vl.saturating_sub(cl) >= CROSS_LAYERS,
                            _ => false,
                        })
                        .count()
                })
                .unwrap_or(0);
            if clen < FRAME_MIN && far <= MAX_COPIES_OF_ONE_CALLEE {
                continue; // cheap enough to copy; localization handles it
            }
            if victim.as_ref().map_or(true, |(best, _)| clen > *best) {
                victim = Some((clen, id.clone()));
            }
        }
        let Some((_, id)) = victim else {
            break;
        };
        // The far calls only. `cut_edge` prunes and renumbers, so one per pass and
        // re-measure: the copy it removes can change every other node's column.
        let Some(index) = edges.iter().position(|edge| {
            edge.to == id
                && match (layer_of.get(id.as_str()), layer_of.get(edge.from.as_str())) {
                    (Some(&vl), Some(&cl)) => vl.saturating_sub(cl) >= CROSS_LAYERS,
                    _ => false,
                }
        }) else {
            break;
        };
        cut_edge(nodes, edges, index);
        done += 1;
    }
    done
}

fn duplicate_crossing_shared(
    nodes: &mut Vec<GraphNode>,
    edges: &mut Vec<GraphEdge>,
    root_id: &str,
) {
    const CROSS_LAYERS: usize = 2; // a caller this many columns back skips a column
    // Copy helpers small enough that a frame of their own would be a husk. The two
    // bands MEET at FRAME_MIN, which is the whole point: under it a crossing callee
    // is copied next to each far caller, at or over it `cut_crossing_shared` has
    // already given it a frame and a card per caller. Nothing falls between them and
    // keeps crossing, which is what the old cap of 3 left behind — a helper with four
    // callees of its own was too big to copy and never big enough to be cut.
    const MAX_CLOSURE: usize = FRAME_MIN - 1;
    // Effectively uncapped copies: a leaf used 56× needs 56 local copies, or it
    // still draws long crossing arrows. Small copies, so the box budget (below)
    // is the real bound.
    const MAX_COPIES: usize = 4096;
    let budget = nodes.len() * 4;
    let mut added = 0usize;
    // Callees already found to have nothing to copy for. Reaching one used to END the
    // whole pass, so the crossing arrows of every candidate ranked after it survived: one
    // frame kept a connector flying over a column after eight copies had been made. A
    // victim that cannot be helped is a reason to look at the next one, not to stop.
    let mut exhausted: HashSet<String> = HashSet::new();

    loop {
        if added >= budget {
            break;
        }
        // Preliminary layout to read each node's column (layer).
        let layout_nodes: Vec<LayoutNode> = nodes
            .iter()
            .map(|node| LayoutNode {
                id: node.id.clone(),
                width: node.board_width(),
                height: node.board_height(),
            })
            .collect();
        let anchors = compute_anchors(nodes, edges);
        let layout_edges: Vec<LayoutEdge> = edges
            .iter()
            .zip(anchors.iter())
            .map(|(edge, anchor)| LayoutEdge {
                from: edge.from.clone(),
                to: edge.to.clone(),
                from_line_fraction: anchor.y_fraction,
            })
            .collect();
        let layout = layout_graph(root_id, &layout_nodes, &layout_edges, LayoutConfig::default());
        let layer_of: HashMap<&str, usize> =
            layout.nodes.iter().map(|p| (p.id.as_str(), p.layer)).collect();

        // Callers + out-adjacency + shared set (recomputed; copies change them).
        let mut callers: HashMap<String, Vec<String>> = HashMap::new();
        let mut out: HashMap<String, Vec<String>> = HashMap::new();
        for edge in edges.iter() {
            callers.entry(edge.to.clone()).or_default().push(edge.from.clone());
            out.entry(edge.from.clone()).or_default().push(edge.to.clone());
        }
        let shared: HashSet<String> = callers
            .iter()
            .filter(|(_, cs)| cs.len() >= 2)
            .map(|(id, _)| id.clone())
            .collect();

        // How far back a caller sits from a node, in columns.
        let skip = |v: &str, c: &str| -> usize {
            match (layer_of.get(v), layer_of.get(c)) {
                (Some(&vl), Some(&cl)) => vl.saturating_sub(cl),
                _ => 0,
            }
        };

        // Copy the SMALLEST crossing helper first (the cheap mesh floor — leaves
        // like sqrt/mul512), then bigger ones; ties broken by the farthest-back
        // caller. Copying the floor first dissolves the mesh from the bottom, which
        // is what a top-down pass could never reach before the budget ran out.
        let mut best: Option<(usize, usize, String)> = None; // (closure_len, -worst, id)
        for v in &shared {
            if exhausted.contains(v) {
                continue;
            }
            let worst = callers
                .get(v)
                .map(|cs| cs.iter().map(|c| skip(v, c)).max().unwrap_or(0))
                .unwrap_or(0);
            if worst < CROSS_LAYERS {
                continue;
            }
            let clen = private_closure(v, &out, &shared).len();
            if clen > MAX_CLOSURE {
                continue;
            }
            // Repeating one callee past this is `cut_crossing_shared`'s business: it
            // gave the callee a frame, and copying it here as well would put the
            // drawing back beside every caller and undo that.
            let far = callers
                .get(v)
                .map(|cs| {
                    let mut distinct: Vec<&String> = cs.iter().collect();
                    distinct.sort();
                    distinct.dedup();
                    distinct.iter().filter(|c| skip(v, c) >= CROSS_LAYERS).count()
                })
                .unwrap_or(0);
            if far > MAX_COPIES_OF_ONE_CALLEE {
                continue;
            }
            // Prefer smaller closure, then larger skip.
            let key = (clen, usize::MAX - worst);
            if best.as_ref().map_or(true, |(cl, w, _)| key < (*cl, *w)) {
                best = Some((key.0, key.1, v.clone()));
            }
        }
        let Some((_, _, victim)) = best else {
            break;
        };
        let closure = private_closure(&victim, &out, &shared);

        // Copy for callers ≥ CROSS_LAYERS back, but never for the NEAREST one — it
        // stays on the original so the victim keeps a caller (and, alone, no longer
        // crosses).
        let mut distinct_callers: Vec<String> = callers.get(&victim).cloned().unwrap_or_default();
        distinct_callers.sort();
        distinct_callers.dedup();
        distinct_callers.sort_by_key(|c| skip(&victim, c)); // nearest first
        let distant: Vec<String> = distinct_callers
            .iter()
            .skip(1)
            .filter(|c| skip(&victim, c) >= CROSS_LAYERS)
            .cloned()
            .collect();
        if distant.is_empty() {
            exhausted.insert(victim.clone());
            continue;
        }

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
        // Keep calls to shared children OUTSIDE the closure, or the copy dead-ends.
        let external: Vec<GraphEdge> = edges
            .iter()
            .filter(|edge| closure.contains(&edge.from) && !closure.contains(&edge.to))
            .cloned()
            .collect();

        let mut made = 0usize;
        for caller in distant.iter() {
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
            // Copy each external call from the copied closure onto the SAME shared
            // target, so the duplicate keeps calling out (no dead-ends).
            for edge in &external {
                let mut copy = edge.clone();
                copy.from = id_map[&edge.from].clone();
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
            scale: 1.0,
            png_path: String::new(),
            png_width: 0,
            png_height: 0,
            rendered_lines: Vec::new(),
            line_offset: 0,
            writes_storage: false,
        write_lines: Vec::new(),
        external_call_lines: Vec::new(),
        leads_to_write: false,
        write_call_lines: Vec::new(),
        external_call_sites: Vec::new(),
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

/// Balanced partitioning, not "cut until under a cap". A frame is aimed AT a
/// readable size, and a graph too big for one frame is split into several pieces
/// each near that size — never one giant frame, never a scatter of husks.
///
/// - `FRAME_TARGET` — the size (in screenshots) a frame is aimed at. ~1500–2600px
///   screenshots across ≤5 layers land around 10–12k px wide: readable at a normal
///   zoom, ~30 connectors. (Auditors rejected 24–33-shot frames as unreadable.)
/// - `FRAME_MAX` — above this (measured as EFFECTIVE size, depth included) a frame
///   is split; a graph at or under it ships whole (one 20-shot frame beats a
///   14 + 6 split with a link card to chase).
/// - `FRAME_MIN` — a cut branch, and the residual left behind, must each keep at
///   least this many of their own screenshots; below it the piece is a husk.
/// - `MAX_CUTS_PER_FRAME` — at most this many branches leave one frame, so a frame
///   never becomes a scavenger hunt of link cards. A cut branch bigger than
///   `FRAME_MAX` becomes its own frame and is split again by the same policy.
const FRAME_TARGET: usize = 15;
const FRAME_MAX: usize = 20;
const FRAME_MIN: usize = 6;
const MAX_CUTS_PER_FRAME: usize = 10;

/// Depth costs horizontal px (the scarce resource): past this many layers a frame
/// runs off-screen even at a modest screenshot count. Effective size scales up
/// `DEPTH_PENALTY` per layer beyond the free budget, so a deep-and-narrow frame is
/// cut sooner than a shallow-and-wide one of the same raw count.
const DEPTH_FREE_LAYERS: usize = 5;
const DEPTH_PENALTY: f64 = 0.15;

/// Replace one call with a card linking to the callee's own frame.
///
/// The decision is per **edge**, not per function. `FeeLib.feeOf` is called from
/// layer 1 and from layer 2 and lays out on layer 3: the arrow from layer 2 is
/// fine, the one from layer 1 reaches further. Moving the whole function out
/// would take away the arrow that was already fine, so only the far call is
/// replaced — the near caller keeps the screenshot.
/// Swaps each callee that already has its own frame for a link card, except when that would
/// leave this frame under `FRAME_MIN` screenshots: a frame that is one screenshot and a couple
/// of link cards shows nothing (the same husk floor as the automatic cut), so such a callee
/// is drawn inline instead. Returns the calls linked and the callees kept inline.
fn link_deployed_frames(
    nodes: &mut Vec<GraphNode>,
    edges: &mut Vec<GraphEdge>,
    deployed_titles: &HashSet<String>,
) -> (usize, Vec<String>) {
    let Some(root_id) = nodes.first().map(|n| n.id.clone()) else {
        return (0, Vec::new());
    };
    let targets: Vec<String> = {
        let mut seen = HashSet::new();
        edges
            .iter()
            .filter(|edge| edge.to != root_id)
            .filter(|edge| {
                nodes.iter().any(|node| {
                    node.id == edge.to
                        && node.kind == NodeKind::Screenshot
                        && deployed_titles.contains(&node.label)
                })
            })
            .filter_map(|edge| seen.insert(edge.to.clone()).then(|| edge.to.clone()))
            .collect()
    };
    let (mut linked, mut kept) = (0usize, Vec::new());
    for target in targets {
        // Already pruned by an earlier link (it was inside that subtree).
        let Some(label) = nodes.iter().find(|n| n.id == target).map(|n| n.label.clone()) else {
            continue;
        };
        let (mut trial_nodes, mut trial_edges) = (nodes.clone(), edges.clone());
        let calls = trial_edges.iter().filter(|e| e.to == target).count();
        cut_node(&mut trial_nodes, &mut trial_edges, &target);
        if screenshot_count(&trial_nodes) < FRAME_MIN {
            kept.push(label);
            continue;
        }
        *nodes = trial_nodes;
        *edges = trial_edges;
        linked += calls;
    }
    (linked, kept)
}

fn cut_edge(nodes: &mut Vec<GraphNode>, edges: &mut Vec<GraphEdge>, index: usize) {
    let Some((label, file)) = nodes
        .iter()
        .find(|node| node.id == edges[index].to)
        .map(|node| (node.label.clone(), node.file_path.clone()))
    else {
        return;
    };

    let card_id = format!("\u{0}link{index}");
    nodes.push(GraphNode {
        id: card_id.clone(),
        label: label.clone(),
        kind: NodeKind::Link { target: label, file },
        file_path: String::new(),
        start_line: 0,
        end_line: 0,
        depth: 0,
        font_size: 22,
        scale: 1.0,
        png_path: String::new(),
        png_width: LINK_CARD_WIDTH as u32,
        png_height: LINK_CARD_HEIGHT as u32,
        rendered_lines: Vec::new(),
        line_offset: 0,
        writes_storage: false,
        write_lines: Vec::new(),
        external_call_lines: Vec::new(),
        leads_to_write: false,
        write_call_lines: Vec::new(),
        external_call_sites: Vec::new(),
    });
    edges[index].to = card_id;
    prune_unreachable(nodes, edges);
}

/// Link out a whole (possibly shared) node: repoint EVERY call into `target_id`
/// onto its own link card, so each caller keeps a nearby card pointing at the
/// node's frame, and the node's now-unreachable subtree is pruned. Unlike
/// [`cut_edge`], which severs a single last-reference call, this removes a node
/// reached from several callers — the only way to carve a balanced piece out of a
/// densely-shared graph, where cutting one edge frees nothing.
fn cut_node(nodes: &mut Vec<GraphNode>, edges: &mut Vec<GraphEdge>, target_id: &str) {
    let Some((label, file)) = nodes
        .iter()
        .find(|node| node.id == target_id)
        .map(|node| (node.label.clone(), node.file_path.clone()))
    else {
        return;
    };
    let in_edges: Vec<usize> = edges
        .iter()
        .enumerate()
        .filter(|(_, e)| e.to == target_id)
        .map(|(i, _)| i)
        .collect();
    for (n, idx) in in_edges.into_iter().enumerate() {
        let card_id = format!("\u{0}linknode_{target_id}_{n}");
        nodes.push(GraphNode {
            id: card_id.clone(),
            label: label.clone(),
            kind: NodeKind::Link { target: label.clone(), file: file.clone() },
            file_path: String::new(),
            start_line: 0,
            end_line: 0,
            depth: 0,
            font_size: 22,
            scale: 1.0,
            png_path: String::new(),
            png_width: LINK_CARD_WIDTH as u32,
            png_height: LINK_CARD_HEIGHT as u32,
            rendered_lines: Vec::new(),
            line_offset: 0,
            writes_storage: false,
        write_lines: Vec::new(),
        external_call_lines: Vec::new(),
        leads_to_write: false,
        write_call_lines: Vec::new(),
        external_call_sites: Vec::new(),
        });
        edges[idx].to = card_id;
    }
    prune_unreachable(nodes, edges);
}

/// Screenshots only: a card is a reference, not something to read.
fn screenshot_count(nodes: &[GraphNode]) -> usize {
    nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Screenshot)
        .count()
}

/// Screenshot count adjusted for depth: past `DEPTH_FREE_LAYERS` layers a frame
/// runs off-screen horizontally, so each extra layer inflates the effective size.
/// A shallow-wide frame reads far better than a deep-narrow one of the same count,
/// and this is what makes the cut policy split the latter sooner.
fn effective_size(nodes: &[GraphNode]) -> usize {
    let count = screenshot_count(nodes);
    let max_layer = nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Screenshot)
        .map(|n| n.depth)
        .max()
        .unwrap_or(0);
    let over = max_layer.saturating_sub(DEPTH_FREE_LAYERS);
    (count as f64 * (1.0 + DEPTH_PENALTY * over as f64)).round() as usize
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
/// Screenshot ids reachable from `root` (root included if it is a screenshot),
/// following edges. A visited set makes cycles/shared subtrees count once.
fn reachable_screens(root: &str, adjacency: &HashMap<&str, Vec<&str>>, screens: &HashSet<&str>) -> HashSet<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: HashSet<String> = HashSet::new();
    let mut stack = vec![root.to_string()];
    while let Some(id) = stack.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        if screens.contains(id.as_str()) {
            out.insert(id.clone());
        }
        for next in adjacency.get(id.as_str()).cloned().unwrap_or_default() {
            stack.push(next.to_string());
        }
    }
    out
}

/// All node ids reachable from `root` (root included), following edges. Used to
/// measure a branch's whole subtree and its edge boundary.
fn reachable_nodes(root: &str, adjacency: &HashMap<&str, Vec<&str>>) -> HashSet<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut stack = vec![root.to_string()];
    while let Some(id) = stack.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        for next in adjacency.get(id.as_str()).cloned().unwrap_or_default() {
            stack.push(next.to_string());
        }
    }
    seen
}

/// Choose the single best branch to link out to its own frame — or `None` when no
/// cut yields a readable, non-husk piece. Rather than lopping off the LARGEST
/// subtree (which leaves two lopsided halves), we score each candidate on how close
/// its size lands to `budget` (the per-piece target), minus the cross-frame edges
/// the cut severs (each becomes a link card, not a drawn arrow), plus small bonuses
/// for a subtree that is widely reused or already has a frame. A branch smaller than
/// `FRAME_MIN`, or one whose removal would leave the frame itself below `FRAME_MIN`,
/// is rejected — that is the husk guard, in both directions.
fn best_cut(
    nodes: &[GraphNode],
    edges: &[GraphEdge],
    framed: &HashSet<&str>,
    budget: usize,
) -> Option<(Vec<GraphNode>, Vec<GraphEdge>)> {
    let has_children: HashSet<&str> = edges.iter().map(|edge| edge.from.as_str()).collect();
    let screens: HashSet<&str> = nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Screenshot)
        .map(|n| n.id.as_str())
        .collect();
    let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in edges.iter() {
        adjacency.entry(edge.from.as_str()).or_default().push(edge.to.as_str());
    }
    let root = nodes.first().map(|n| n.id.as_str()).unwrap_or("");

    let before = screenshot_count(nodes);
    let budget_f = budget.max(1) as f64;
    let mut best: Option<(f64, Vec<GraphNode>, Vec<GraphEdge>)> = None;

    // Candidates are NODES with a subtree (not the root, not a leaf, not a link
    // card). Cutting a node lifts out its whole subtree — as one frame — no matter
    // how many callers reach it, which is what lets a densely-shared graph be
    // partitioned at all.
    for node in nodes.iter() {
        if node.kind != NodeKind::Screenshot
            || node.id == root
            || !has_children.contains(node.id.as_str())
        {
            continue;
        }
        let has_frame = framed.contains(node.label.as_str());
        let sub = reachable_screens(&node.id, &adjacency, &screens).len();

        let mut candidate_nodes = nodes.to_vec();
        let mut candidate_edges = edges.to_vec();
        cut_node(&mut candidate_nodes, &mut candidate_edges, &node.id);
        let saved = before.saturating_sub(screenshot_count(&candidate_nodes));
        if saved == 0 {
            continue;
        }
        let residual = before.saturating_sub(saved);

        // Husk guard, both directions: neither the new piece nor the leftover frame
        // may fall below the readable minimum. An existing frame is exempt on the
        // piece side (no new frame is created — the cut only replaces arrows).
        if (!has_frame && sub < FRAME_MIN) || residual < FRAME_MIN {
            continue;
        }

        // Cross-frame edges this cut severs (the subtree's boundary) — each becomes
        // a link card rather than a drawn arrow — and how many callers reach it.
        let subtree = reachable_nodes(&node.id, &adjacency);
        let severed = edges
            .iter()
            .filter(|e| subtree.contains(&e.from) != subtree.contains(&e.to))
            .count()
            .max(1);
        let reuse = edges.iter().filter(|e| e.to == node.id).count();

        // Nearness to the target dominates; severed edges tiebreak; reuse and an
        // existing frame nudge. (Weights: size distance ~3.3/shot, edge ~12.)
        let score = -(50.0 / budget_f) * (sub as f64 - budget_f).abs()
            - 12.0 * (severed as f64 - 1.0)
            + 10.0 * (reuse.min(3) as f64)
            + if has_frame { 8.0 } else { 0.0 };
        if best.as_ref().map(|(most, _, _)| score > *most).unwrap_or(true) {
            best = Some((score, candidate_nodes, candidate_edges));
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
            scale: 1.0,
            png_path: String::new(),
            png_width: 1000,
            png_height: 300,
            rendered_lines: Vec::new(),
            line_offset: 0,
            writes_storage: false,
        write_lines: Vec::new(),
        external_call_lines: Vec::new(),
        leads_to_write: false,
        write_call_lines: Vec::new(),
        external_call_sites: Vec::new(),
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

    /// A callee with its own frame becomes a link card only while the frame keeps
    /// `FRAME_MIN` screenshots: `priced` → `book` + `priceIn`, both framed, used to end
    /// up as one screenshot and two cards.
    #[test]
    fn linking_to_deployed_frames_never_leaves_a_husk() {
        // priced → book → b1..b5 ; priced → priceIn → p1..p8
        let mut nodes = vec![node("priced"), node("book"), node("priceIn")];
        let mut edges = vec![edge("priced", "book"), edge("priced", "priceIn")];
        for i in 1..=5 {
            nodes.push(node(&format!("b{i}")));
            edges.push(edge("book", &format!("b{i}")));
        }
        for i in 1..=8 {
            nodes.push(node(&format!("p{i}")));
            edges.push(edge("priceIn", &format!("p{i}")));
        }
        let framed: HashSet<String> = ["book", "priceIn"].iter().map(|s| s.to_string()).collect();
        let (linked, kept) = link_deployed_frames(&mut nodes, &mut edges, &framed);
        // book goes out (10 screenshots stay), priceIn would leave 1: drawn inline.
        assert_eq!(linked, 1);
        assert_eq!(kept, vec!["priceIn".to_string()]);
        assert_eq!(screenshot_count(&nodes), 10);
        assert!(!nodes.iter().any(|n| n.id == "b1"));
        assert!(nodes.iter().any(|n| n.id == "p1"));
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

        if let Some((cut_nodes, _)) = best_cut(&nodes, &edges, &HashSet::new(), FRAME_TARGET) {
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

    /// A graph too small to split is left alone: the husk guard wins over the cut.
    ///
    /// Cutting root→a would strand `b` with it, which frees two screenshots — but both
    /// the piece (2) and the residual (1) land under `FRAME_MIN`, and a frame holding one
    /// screenshot that points at a frame holding two is a scavenger hunt, not a diagram.
    /// So there is no candidate at all.
    #[test]
    fn test_a_graph_below_the_husk_floor_is_not_cut() {
        let nodes = vec![node("root"), node("a"), node("b")];
        let edges = vec![edge("root", "a"), edge("a", "b")];

        assert!(nodes.len() < FRAME_MIN);
        assert!(best_cut(&nodes, &edges, &HashSet::new(), FRAME_TARGET).is_none());
    }
}

/// Frame URL for every function a card in this graph points at.
///
/// A function that already has a frame is reused; one that does not is deployed
/// first, so the card has somewhere to go. Distinct cards for the same function
/// collapse to one lookup, which is what makes several diagrams share a helper's
/// frame instead of each building its own.
/// The cards of a frame, in the order a reader meets them.
///
/// Depth-first from the root, each node's calls taken in source order, following a drawn
/// callee as soon as the line that calls it is read — which is how the source is read and
/// how the frame is laid out (callees in call order, top-aligned). The deploy follows this
/// list, so a branch is finished before the next one starts and the frames appear in the
/// order the reader will walk them.
///
/// It replaced the order the cards happened to sit in `nodes`, which is the order
/// `best_cut` scored them: deterministic, and unrelated to anything a reader does.
fn card_reading_order(
    nodes: &[GraphNode],
    edges: &[GraphEdge],
    root_id: &str,
) -> Vec<(String, String)> {
    let mut out_edges: HashMap<&str, Vec<&GraphEdge>> = HashMap::new();
    for edge in edges {
        out_edges.entry(edge.from.as_str()).or_default().push(edge);
    }
    for calls in out_edges.values_mut() {
        calls.sort_by_key(|edge| (edge.line_in_slice, edge.column));
    }
    let by_id: HashMap<&str, &GraphNode> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    let mut ordered: Vec<(String, String)> = Vec::new();
    let mut listed: HashSet<String> = HashSet::new();
    let mut visited: HashSet<String> = HashSet::new();
    // Recursive rather than a stack: a card is listed the moment it is met, while a drawn
    // callee is descended into, and a stack can only do one of those in order — reversing
    // it to fix the descent puts the cards back to front.
    fn walk(
        id: &str,
        out_edges: &HashMap<&str, Vec<&GraphEdge>>,
        by_id: &HashMap<&str, &GraphNode>,
        visited: &mut HashSet<String>,
        listed: &mut HashSet<String>,
        ordered: &mut Vec<(String, String)>,
    ) {
        if !visited.insert(id.to_string()) {
            return;
        }
        let Some(calls) = out_edges.get(id) else {
            return;
        };
        for edge in calls {
            match by_id.get(edge.to.as_str()).map(|node| &node.kind) {
                Some(NodeKind::Link { target, file }) => {
                    if listed.insert(target.clone()) {
                        ordered.push((target.clone(), file.clone()));
                    }
                }
                Some(NodeKind::Screenshot) => {
                    walk(&edge.to, out_edges, by_id, visited, listed, ordered)
                }
                None => {}
            }
        }
    }
    walk(root_id, &out_edges, &by_id, &mut visited, &mut listed, &mut ordered);

    // A card the walk could not reach cannot exist (`prune_unreachable` runs after every
    // cut), but losing one would silently drop a frame, so they are appended rather than
    // trusted away.
    for node in nodes {
        if let NodeKind::Link { target, file } = &node.kind {
            if listed.insert(target.clone()) {
                ordered.push((target.clone(), file.clone()));
            }
        }
    }
    ordered
}


/// The frame URL for a function, if it is registered **and** still on the board.
///
/// A registry entry outlives the frame it names: boards are edited by hand, and
/// deleting a frame in Miro leaves the entry behind. Checking the board costs
/// one read and turns "you already deployed this" into something true, rather
/// than a refusal to redeploy what is no longer there. A stale entry is dropped
/// on the way past, so the question is only asked once.
/// `bat-cli relink`: point a record at the frame it belongs to, or report the drift.
///
/// Arranging a board is part of reading it, and cutting a frame and pasting it elsewhere gives
/// every widget a new id — so the registry ends up describing frames that no longer exist,
/// while the frames themselves are right there under the same titles. Deploying again would
/// fix the record and lose the arrangement, which is the wrong trade.
pub async fn run_relink(
    entry_point: Option<String>,
    deployment: Option<String>,
    frame_url: Option<String>,
    check: bool,
) -> Result<()> {
    let client = MiroClient::new_refreshed()
        .await
        .change_context(EvmMiroError)?;
    let metadata = EvmBatMetadata::read_metadata().change_context(EvmMiroError)?;

    if check || entry_point.is_none() {
        let frames = client.list_frames().await.change_context(EvmMiroError)?;
        let mut alive = 0usize;
        let mut fixable: Vec<(String, usize)> = Vec::new();
        let mut gone: Vec<String> = Vec::new();
        for record in &metadata.miro.auto.frames {
            if frames.iter().any(|frame| frame.id == record.frame_id) {
                alive += 1;
                continue;
            }
            let wanted = format!("auto: {}", record.entry_point);
            let matches = frames.iter().filter(|frame| frame.title == wanted).count();
            if matches == 0 {
                gone.push(record.entry_point.clone());
            } else {
                fixable.push((record.entry_point.clone(), matches));
            }
        }
        println!("{} frame(s) recorded, {alive} still where the registry says", metadata.miro.auto.frames.len());
        for (entry_point, matches) in &fixable {
            if *matches == 1 {
                println!(
                    "  {} {} moved — {} re-anchors it",
                    "↻".yellow(),
                    entry_point,
                    format!("bat-cli relink {entry_point}").green()
                );
            } else {
                println!(
                    "  {} {} has {matches} frames with its title — {} picks one",
                    "?".yellow(),
                    entry_point,
                    format!("bat-cli relink {entry_point} --frame-url <url>").green()
                );
            }
        }
        for entry_point in &gone {
            println!("  {} {} is not on the board at all", "✗".red(), entry_point);
        }
        if fixable.is_empty() && gone.is_empty() {
            println!("  {} nothing to relink", "✓".green());
        }
        return Ok(());
    }

    let entry_point = entry_point.expect("checked above");
    // Which deployment's copy. A function reached by three entry points has three records
    // under this name, and relinking "the one called X" used to take whichever came first
    // — which is how two records ended up pointing at one frame, leaving the pair
    // indistinguishable even by id.
    let candidates: Vec<&AutoDeployedFrame> = metadata
        .miro
        .auto
        .frames
        .iter()
        .filter(|frame| {
            frame.entry_point == entry_point
                && deployment.as_ref().is_none_or(|root| &frame.cluster_root == root)
        })
        .collect();
    if deployment.is_none() && candidates.len() > 1 {
        let listed = candidates
            .iter()
            .map(|frame| format!("    {} ({})", frame.cluster_root, frame.frame_url))
            .collect::<Vec<_>>()
            .join("\n");
        return Err(Report::new(EvmMiroError)
            .attach_printable(format!(
                "`{entry_point}` was drawn by {} deployments:\n{listed}",
                candidates.len()
            ))
            .attach(crate::Suggestion(
                "say which one, with --deployment <entry point>".to_string(),
            )));
    }
    let Some(record) = candidates.first().map(|frame| (*frame).clone()) else {
        return Err(Report::new(EvmMiroError)
            .attach_printable(format!("no frame recorded for `{entry_point}`"))
            .attach(crate::Suggestion(
                "run `bat-cli relink --check` to see what is recorded".to_string(),
            )));
    };

    // Named frame: take the widget id straight out of the URL the board hands out.
    if let Some(url) = frame_url {
        let Some(id) = url
            .split("moveToWidget=")
            .nth(1)
            .map(|rest| rest.split(['&', '#']).next().unwrap_or(rest).to_string())
        else {
            return Err(Report::new(EvmMiroError)
                .attach_printable(format!("no `moveToWidget=` id in `{url}`"))
                .attach(crate::Suggestion(
                    "copy the frame's link from Miro (right-click → Copy link)".to_string(),
                )));
        };
        let frames = client.list_frames().await.change_context(EvmMiroError)?;
        let Some(frame) = frames.iter().find(|frame| frame.id == id) else {
            return Err(Report::new(EvmMiroError)
                .attach_printable(format!("no frame with id {id} on this board")));
        };
        let record = rebuild_record(record, frame, &client).await?;
        println!(
            "{} {} → {}",
            "✓".green(),
            entry_point.bold(),
            record.frame_url.blue()
        );
        return Ok(());
    }

    match reanchor_frame(&entry_point, deployment.as_deref(), &client).await? {
        Some(record) => {
            println!(
                "{} {} → {}",
                "✓".green(),
                entry_point.bold(),
                record.frame_url.blue()
            );
            Ok(())
        }
        None => Err(Report::new(EvmMiroError)
            .attach_printable(format!(
                "no frame titled `auto: {entry_point}` on the board"
            ))
            .attach(crate::Suggestion(
                "pass --frame-url <url> if it was renamed, or deploy it again".to_string(),
            ))),
    }
}

/// The label a node's screenshot was uploaded with, which is what survives on the board.
///
/// Node ids are `Contract::function` (plus `@line` when the name is overloaded); the image's
/// title is the dotted `Contract.function`. A link card has neither — it is a shape, and the
/// board keeps no title for it, so it cannot be recognised after a paste.
fn label_for_node(node_id: &str) -> Option<String> {
    if node_id.starts_with('\u{0}') {
        return None;
    }
    let base = node_id.split('@').next().unwrap_or(node_id);
    Some(base.replace("::", "."))
}

/// Re-point a frame's record at the copy that is actually on the board.
///
/// Cutting and pasting a frame in Miro gives it and every child a NEW id, which orphans the
/// registry — and dragging does not, so the difference is invisible to whoever is arranging
/// the board. The paste keeps the titles, though: the frame is still `auto: <entry point>`
/// and each screenshot still carries its function's label, so the record can be rebuilt from
/// what is there.
///
/// Shapes cannot: link cards, connector markers and borders carry no title. Their recorded
/// ids are dropped, which is safe because `undeploy` deletes the frame's live children rather
/// than the ids it once wrote down.
async fn reanchor_frame(
    title: &str,
    cluster_root: Option<&str>,
    client: &MiroClient,
) -> Result<Option<AutoDeployedFrame>> {
    let record = {
        let metadata = EvmBatMetadata::read_metadata().change_context(EvmMiroError)?;
        metadata
            .miro
            .auto
            .frames
            .iter()
            .find(|frame| {
                frame.entry_point == title
                    && cluster_root.is_none_or(|root| frame.cluster_root == root)
            })
            .cloned()
    };
    let Some(record) = record else {
        return Ok(None);
    };

    let wanted = format!("auto: {title}");
    let frames = client.list_frames().await.change_context(EvmMiroError)?;
    let candidates: Vec<_> = frames.iter().filter(|frame| frame.title == wanted).collect();

    match candidates.len() {
        0 => Ok(None),
        // A duplicated frame (or a paste whose original is still there) makes this a
        // question about which one the auditor means, and that is theirs to answer.
        _ if candidates.len() > 1 => Err(Report::new(EvmMiroError)
            .attach_printable(format!(
                "{} frames on the board are titled `{wanted}`:\n    {}",
                candidates.len(),
                candidates
                    .iter()
                    .map(|frame| client.frame_url(&frame.id))
                    .collect::<Vec<_>>()
                    .join("\n    ")
            ))
            .attach(crate::Suggestion(format!(
                "pick one: `bat-cli relink {title} --frame-url <url>`"
            )))),
        _ => {
            let frame = candidates[0];
            let record = rebuild_record(record, frame, client).await?;
            println!(
                "  {} re-anchored {} to the frame now on the board (its id changed, so it was cut and pasted)",
                "↻".yellow(),
                title.bold()
            );
            Ok(Some(record))
        }
    }
}

/// Rewrite a record against a frame that is really on the board, matching every screenshot by
/// the title it carries.
async fn rebuild_record(
    mut record: AutoDeployedFrame,
    frame: &crate::batbelt::miro::client::BoardFrame,
    client: &MiroClient,
) -> Result<AutoDeployedFrame> {
    let children = client
        .frame_children(&frame.id, (frame.x, frame.y, frame.width, frame.height))
        .await
        .unwrap_or_default();
    let by_title: HashMap<&str, &crate::batbelt::miro::client::FrameChild> = children
        .iter()
        .filter(|child| !child.title.is_empty())
        .map(|child| (child.title.as_str(), child))
        .collect();

    let mut images = Vec::new();
    let mut image_dims = Vec::new();
    let mut node_positions = Vec::new();
    for (node_id, _) in &record.images {
        let Some(label) = label_for_node(node_id) else {
            continue;
        };
        let Some(child) = by_title.get(label.as_str()) else {
            continue;
        };
        images.push((node_id.clone(), child.id.clone()));
        image_dims.push((node_id.clone(), child.width as u32, child.height as u32));
        node_positions.push((node_id.clone(), child.x, child.y));
    }
    let screenshots = record
        .screenshots
        .iter()
        .filter_map(|shot| {
            by_title.get(shot.label.as_str()).map(|child| {
                crate::batbelt::evm::metadata::bat_metadata::ExtraScreenshot {
                    label: shot.label.clone(),
                    item_id: child.id.clone(),
                    x: child.x,
                    y: child.y,
                    width: child.width,
                    height: child.height,
                    with_documentation: shot.with_documentation,
                }
            })
        })
        .collect();

    record.frame_id = frame.id.clone();
    record.frame_url = client.frame_url(&frame.id);
    record.x = frame.x;
    record.y = frame.y;
    record.width = frame.width;
    record.height = frame.height;
    record.images = images;
    record.image_dims = image_dims;
    record.node_positions = node_positions;
    record.screenshots = screenshots;
    // Untitled items (link cards, markers, borders) and the connectors between them cannot be
    // recognised on the pasted copy. Their old ids point at nothing, so drop them rather than
    // keep a list that would delete items on somebody else's frame.
    record.link_cards.clear();
    record.callee_connectors.clear();
    record.connector_ids.clear();
    record.marker_ids.clear();
    record.border_ids.clear();
    save_frame_record(&record)?;
    Ok(record)
}

/// The record for `title`, re-anchored if the board moved underneath it, or `None` when the
/// frame really is gone (in which case the record is forgotten, as before).
pub(crate) async fn ensure_frame_record(
    title: &str,
    // Which deployment's copy of `title`. A function reached by three entry points has
    // three frames with this name, and a lookup without this answered with whichever
    // record came first in the file — an order every save changes, by removing a record
    // and pushing it back at the end. So the same command run twice drew on two different
    // frames. `None` is for the callers that genuinely mean "the only one", and still
    // stops when the name turns out to be ambiguous.
    cluster_root: Option<&str>,
    client: &MiroClient,
) -> Result<Option<AutoDeployedFrame>> {
    let record = {
        let metadata = EvmBatMetadata::read_metadata().change_context(EvmMiroError)?;
        let matching: Vec<&AutoDeployedFrame> = metadata
            .miro
            .auto
            .frames
            .iter()
            .filter(|frame| {
                frame.entry_point == title
                    && cluster_root.is_none_or(|root| frame.cluster_root == root)
            })
            .collect();
        if cluster_root.is_none() && matching.len() > 1 {
            let candidates = matching
                .iter()
                .map(|frame| format!("    {} ({})", frame.cluster_root, frame.frame_url))
                .collect::<Vec<_>>()
                .join("\n");
            return Err(Report::new(EvmMiroError)
                .attach_printable(format!(
                    "`{title}` was drawn by {} deployments:\n{candidates}",
                    matching.len()
                ))
                .attach(crate::Suggestion(
                    "say which one, with --deployment <entry point>".to_string(),
                )));
        }
        matching.first().map(|frame| (*frame).clone())
    };
    let Some(record) = record else {
        return Ok(None);
    };
    match client.item_status(&record.frame_id).await {
        Some(true) => return Ok(Some(record)),
        // The board could not be asked. Keeping the record is the only safe answer: it is
        // the thing that knows where the frame is, and the frame is not what is in doubt.
        None => {
            println!(
                "  {} could not check whether {}'s frame is still on the board; keeping the record",
                "note:".yellow(),
                title
            );
            return Ok(Some(record));
        }
        Some(false) => {}
    }
    if let Some(reanchored) = reanchor_frame(title, cluster_root, client).await? {
        return Ok(Some(reanchored));
    }

    println!(
        "  {} the frame recorded for {} is gone from the board; forgetting it",
        "note:".yellow(),
        title
    );
    let owner = title.to_string();
    let deployment = cluster_root.map(|root| root.to_string());
    EvmBatMetadata::update_metadata(move |metadata| {
        metadata.miro.auto.frames.retain(|frame| {
            frame.entry_point != owner
                || deployment.as_ref().is_some_and(|root| &frame.cluster_root != root)
        });
    })
    .change_context(EvmMiroError)?;
    Ok(None)
}

#[cfg(test)]
mod ignore_test {
    use super::matches_ignore;

    #[test]
    fn a_name_matches_exactly_and_a_path_by_whole_segments() {
        // What the auditor meant.
        assert!(matches_ignore("Math", "Math", "./lib/oz/contracts/utils/math/Math.sol"));
        assert!(matches_ignore(
            "openzeppelin-contracts/contracts/utils",
            "Math",
            "./lib/openzeppelin-contracts/contracts/utils/math/Math.sol"
        ));
        assert!(matches_ignore("utils/math", "Math", "./lib/oz/contracts/utils/math/Math.sol"));

        // What it must NOT take with it: a contract whose name merely ends in the pattern,
        // which is how `ignore Math` silently removed CollRebalancerMath from a diagram.
        assert!(!matches_ignore(
            "Math",
            "CollRebalancerMath",
            "./src/hooks/everlong/lev/CollRebalancerMath.sol"
        ));
        assert!(!matches_ignore("Curve", "AlmCurve", "./src/hooks/everlong/AlmCurve.sol"));
        // Nor a directory that merely starts with it.
        assert!(!matches_ignore("math", "X", "./src/mathlib/X.sol"));
        assert!(!matches_ignore("", "X", "./src/X.sol"));
    }
}

#[cfg(test)]
mod color_test {
    use super::*;

    fn conflicts<'a>(pairs: &[(&'a str, &'a str)]) -> HashMap<&'a str, HashSet<&'a str>> {
        let mut map: HashMap<&str, HashSet<&str>> = HashMap::new();
        for (a, b) in pairs {
            map.entry(a).or_default().insert(b);
            map.entry(b).or_default().insert(a);
        }
        map
    }

    /// The invariant the whole thing exists for: two arrows a reader compares are never
    /// the same colour.
    #[test]
    fn conflicting_callees_never_share_a_colour() {
        let graph = conflicts(&[("a", "b"), ("b", "c"), ("c", "a"), ("c", "d")]);
        let chosen = color_callees(&graph, |_| (0..DEPTH_COLORS.len()).collect());
        assert!(
            chosen.values().all(|(_, stroke)| *stroke == ConnectorStroke::Solid),
            "four callees fit in a palette of {}",
            DEPTH_COLORS.len()
        );
        for (callee, rivals) in &graph {
            for rival in rivals {
                assert_ne!(
                    chosen[callee], chosen[rival],
                    "{callee} and {rival} are compared and share a colour"
                );
            }
        }
    }

    /// A callee reached from several places keeps ONE colour — that is what makes a
    /// helper recognisable across a frame — so the rule is about conflicts, not arrows.
    #[test]
    fn a_callee_has_exactly_one_colour() {
        let graph = conflicts(&[("helper", "a"), ("helper", "b")]);
        let chosen = color_callees(&graph, |_| (0..DEPTH_COLORS.len()).collect());
        assert!(chosen.contains_key("helper"));
        assert_eq!(chosen.len(), 3);
    }

    /// More mutually-compared functions than colours: the repeats take the second stroke,
    /// so the PAIR is still unique. They used to share one colour AND one stroke, which is
    /// how `cross` and `deployPool` ended up with identical arrows in the same column.
    #[test]
    fn a_palette_that_runs_out_keeps_every_pair_distinct() {
        let names: Vec<String> = (0..DEPTH_COLORS.len() + 2).map(|i| format!("f{i}")).collect();
        let mut pairs: Vec<(&str, &str)> = Vec::new();
        for i in 0..names.len() {
            for j in (i + 1)..names.len() {
                pairs.push((names[i].as_str(), names[j].as_str()));
            }
        }
        let graph = conflicts(&pairs);
        let chosen = color_callees(&graph, |_| (0..DEPTH_COLORS.len()).collect());
        assert_eq!(chosen.len(), DEPTH_COLORS.len() + 2);

        let marks: HashSet<(usize, ConnectorStroke)> = chosen.values().copied().collect();
        assert_eq!(marks.len(), chosen.len(), "two callees share a colour AND a stroke");
        assert_eq!(
            chosen.values().filter(|(_, s)| *s == ConnectorStroke::Dotted).count(),
            2,
            "a clique of {} in a palette of {} spills exactly twice",
            names.len(),
            DEPTH_COLORS.len()
        );
    }

    /// Dashed means "this edge goes backwards". The overflow must not borrow it, or a
    /// repeated colour reads as a cycle.
    #[test]
    fn the_overflow_never_uses_the_cycle_stroke() {
        let names: Vec<String> = (0..DEPTH_COLORS.len() + 1).map(|i| format!("f{i}")).collect();
        let mut pairs: Vec<(&str, &str)> = Vec::new();
        for i in 0..names.len() {
            for j in (i + 1)..names.len() {
                pairs.push((names[i].as_str(), names[j].as_str()));
            }
        }
        let chosen = color_callees(&conflicts(&pairs), |_| (0..DEPTH_COLORS.len()).collect());
        assert!(chosen.values().all(|(_, s)| *s != ConnectorStroke::Dashed));
    }
}

#[cfg(test)]
mod reading_order_test {
    use super::*;

    fn screenshot(id: &str) -> GraphNode {
        GraphNode {
            id: id.to_string(),
            label: id.replace("::", "."),
            kind: NodeKind::Screenshot,
            file_path: String::new(),
            start_line: 1,
            end_line: 2,
            depth: 0,
            font_size: 32,
            scale: 1.0,
            png_path: String::new(),
            png_width: 0,
            png_height: 0,
            rendered_lines: Vec::new(),
            line_offset: 0,
            writes_storage: false,
            write_lines: Vec::new(),
            external_call_lines: Vec::new(),
            leads_to_write: false,
            write_call_lines: Vec::new(),
            external_call_sites: Vec::new(),
        }
    }

    fn card(id: &str, target: &str) -> GraphNode {
        let mut node = screenshot(id);
        node.kind = NodeKind::Link {
            target: target.to_string(),
            file: format!("./src/{target}.sol"),
        };
        node
    }

    fn edge(from: &str, to: &str, line: usize) -> GraphEdge {
        GraphEdge {
            from: from.to_string(),
            to: to.to_string(),
            line_in_slice: line,
            column: 0,
            symbol: to.to_string(),
        }
    }

    /// The reader goes down the root, and follows a callee AS the line that calls it is
    /// read: a card of a callee called early comes before a card the root calls later.
    #[test]
    fn cards_come_in_the_order_a_reader_meets_them() {
        let nodes = vec![
            screenshot("R"),
            screenshot("A"),
            card("c_late", "Late"),
            card("c_deep", "Deep"),
        ];
        let edges = vec![
            edge("R", "A", 10),
            edge("R", "c_late", 20),
            edge("A", "c_deep", 5),
        ];
        let order = card_reading_order(&nodes, &edges, "R");
        assert_eq!(
            order.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(),
            vec!["Deep", "Late"],
            "the card inside the first callee is met before the root's later call"
        );
    }

    /// Calls on one node are read top to bottom, whatever order the edges were built in.
    #[test]
    fn calls_are_read_top_to_bottom() {
        let nodes = vec![screenshot("R"), card("c1", "Third"), card("c2", "First"), card("c3", "Second")];
        let edges = vec![edge("R", "c1", 30), edge("R", "c2", 10), edge("R", "c3", 20)];
        let order = card_reading_order(&nodes, &edges, "R");
        assert_eq!(
            order.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(),
            vec!["First", "Second", "Third"]
        );
    }

    /// One frame per target, however many cards point at it, and a cycle terminates.
    #[test]
    fn a_target_is_listed_once_and_a_cycle_ends() {
        let nodes = vec![
            screenshot("R"),
            screenshot("A"),
            card("c1", "Shared"),
            card("c2", "Shared"),
        ];
        let edges = vec![
            edge("R", "A", 10),
            edge("A", "R", 1), // the cycle
            edge("R", "c1", 20),
            edge("A", "c2", 5),
        ];
        let order = card_reading_order(&nodes, &edges, "R");
        assert_eq!(order.len(), 1, "one frame for one target: {order:?}");
        assert_eq!(order[0].0, "Shared");
    }

    /// A card nothing reaches is still deployed: losing it would silently drop a frame.
    #[test]
    fn an_unreachable_card_is_not_dropped() {
        let nodes = vec![screenshot("R"), card("orphan", "Orphan")];
        let order = card_reading_order(&nodes, &[], "R");
        assert_eq!(order.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(), vec!["Orphan"]);
    }
}

#[cfg(test)]
mod signature_test {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(|l| l.to_string()).collect()
    }

    /// Parameters first, then the locals that are used most — and the ranking counts WHOLE
    /// words, or a one-letter name wins every function by appearing inside `if` and
    /// `feeWad`.
    #[test]
    fn locals_fill_the_slots_the_parameters_leave_busiest_first() {
        let slice = lines(
            "    function f(uint256 amount) internal {\n        uint256 rare = 1;\n        uint256 often = 2;\n        often = often + often + rare;\n    }",
        );
        let traced = traced_names(&slice);
        let names: Vec<&str> = traced.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["amount", "often", "rare"]);
        assert_eq!(traced[0].kind, crate::batbelt::silicon::TraceKind::Parameter);
        assert_eq!(traced[1].kind, crate::batbelt::silicon::TraceKind::Local);
    }

    /// A tuple declaration declares just as much as a single one; it used to be invisible.
    #[test]
    fn a_tuple_declaration_is_traced_too() {
        let slice = lines(
            "    function f() internal {\n        (uint256 p0, uint48 ts) = Store.price();\n        use(p0, ts, p0);\n    }",
        );
        let names: Vec<String> = traced_names(&slice).into_iter().map(|t| t.name).collect();
        assert!(names.contains(&"p0".to_string()), "{names:?}");
        assert!(names.contains(&"ts".to_string()), "{names:?}");
    }

    /// A loop counter lives in its own header; following it is the one thing nobody needs
    /// help with, and leaving it out frees a colour for a name that is genuinely hard to
    /// follow. The bound of the loop is not a counter and stays.
    #[test]
    fn a_loop_counter_is_not_traced() {
        let slice = lines(
            "    function f() internal {\n        uint256 n = size();\n        for (uint256 i = 0; i < n; ++i) {\n            use(i, n);\n        }\n    }",
        );
        let names: Vec<String> = traced_names(&slice).into_iter().map(|t| t.name).collect();
        assert!(!names.contains(&"i".to_string()), "{names:?}");
        assert!(names.contains(&"n".to_string()), "{names:?}");
    }

    /// Only the counter of a `for`, not any variable that happens to be called `i`.
    #[test]
    fn a_variable_named_like_a_counter_is_still_traced() {
        let slice = lines(
            "    function f() internal {\n        uint256 i = pick();\n        use(i, i);\n    }",
        );
        let names: Vec<String> = traced_names(&slice).into_iter().map(|t| t.name).collect();
        assert!(names.contains(&"i".to_string()), "{names:?}");
    }

    /// Nothing is ever left unmarked. Past the palette the colours start over and the rule
    /// alternates solid/broken, so two names may end up alike — which a reader can work out
    /// from context, unlike a name with no mark at all.
    #[test]
    fn more_names_than_colours_are_all_still_marked() {
        use crate::batbelt::silicon::{TraceKind, UNDERLINED_TRACE_COLORS};
        // Past twice the palette, so the cycle has to come round rather than stop.
        let count = UNDERLINED_TRACE_COLORS.len() * 2 + 3;
        let mut body = String::from("    function f() internal {\n");
        for i in 0..count {
            body.push_str(&format!("        uint256 v{i} = {i};\n        use(v{i}, v{i});\n"));
        }
        body.push_str("    }");
        let traced = traced_names(&lines(&body));
        assert_eq!(traced.len(), count, "every local is marked");

        // The first palette-worth is solid, the second broken, and then it starts over:
        // a mark repeats instead of a name going unmarked.
        let solid = traced.iter().filter(|t| !t.dotted).count();
        assert_eq!(solid, UNDERLINED_TRACE_COLORS.len() + 3);
        assert_eq!(traced.iter().filter(|t| t.dotted).count(), UNDERLINED_TRACE_COLORS.len());
        assert!(traced.iter().all(|t| t.color < UNDERLINED_TRACE_COLORS.len()));
        let _ = TraceKind::Local;
    }

    #[test]
    fn more_parameters_than_colours_are_all_still_marked() {
        use crate::batbelt::silicon::TRACE_COLORS;
        let count = TRACE_COLORS.len() + 2;
        let params: Vec<String> = (0..count).map(|i| format!("uint256 a{i}")).collect();
        let slice = lines(&format!("    function f({}) internal {{}}", params.join(", ")));
        let traced = traced_names(&slice);
        assert_eq!(traced.len(), count, "every parameter is marked");
        assert!(traced.iter().all(|t| t.color < TRACE_COLORS.len()));
    }

    /// Yul declares with `let`, and it is not in the Solidity statement tree: in
    /// `FixedPointMathLib.lnWad` the two busiest names in the function are declared there.
    #[test]
    fn a_yul_declaration_is_traced() {
        let slice = lines(
            "    function f(int256 x) internal pure returns (int256 r) {\n        assembly {\n            let p := sub(x, 1)\n            p := mul(p, p)\n            r := p\n        }\n    }",
        );
        let names: Vec<String> = traced_names(&slice).into_iter().map(|t| t.name).collect();
        assert!(names.contains(&"p".to_string()), "{names:?}");
    }

    #[test]
    fn several_names_declared_at_once_in_yul_are_all_traced() {
        let slice = lines(
            "    function f() internal {\n        assembly {\n            let a, b := g()\n            use(a, b)\n        }\n    }",
        );
        let names: Vec<String> = traced_names(&slice).into_iter().map(|t| t.name).collect();
        assert!(names.contains(&"a".to_string()) && names.contains(&"b".to_string()), "{names:?}");
    }

    /// Both kinds opening on the same hue put a salmon `x` and a salmon `r` on every line
    /// of `lnWad`, where the rule under a one-character name is too small to separate them.
    #[test]
    fn the_underlined_sequence_starts_past_the_parameters() {
        let slice = lines(
            "    function f(int256 x) internal pure returns (int256 r) {\n        r = x;\n    }",
        );
        let traced = traced_names(&slice);
        let parameter = traced.iter().find(|t| t.name == "x").expect("x is traced");
        let carried = traced.iter().find(|t| t.name == "r").expect("r is traced");
        assert_ne!(
            crate::batbelt::silicon::palette(parameter.kind)[parameter.color],
            crate::batbelt::silicon::palette(carried.kind)[carried.color],
        );
    }

    /// The value the function is building is the hardest thread to hold, and it never
    /// takes a colour one of the locals already has.
    #[test]
    fn a_named_return_is_traced_apart_from_the_locals() {
        use crate::batbelt::silicon::TraceKind;
        let slice = lines(
            "    function f(uint256 a) internal returns (Plan memory p) {\n        uint256 x = a;\n        p.one = x;\n        p.two = x;\n    }",
        );
        let traced = traced_names(&slice);
        let carried = traced
            .iter()
            .find(|t| t.kind == TraceKind::NamedReturn)
            .expect("the named return is traced");
        assert_eq!(carried.name, "p");
        for local in traced.iter().filter(|t| t.kind == TraceKind::Local) {
            assert_ne!(local.color, carried.color, "{} reuses the return's colour", local.name);
        }
    }

    /// The palette bounds each KIND separately: the decoration tells a parameter from a
    /// local, so the same colour serves one of each.
    #[test]
    fn a_parameter_and_a_local_may_share_a_colour() {
        use crate::batbelt::silicon::TraceKind;
        let slice = lines(
            "    function f(uint256 a) internal {\n        uint256 b = a;\n        use(b, b);\n    }",
        );
        let traced = traced_names(&slice);
        assert_eq!(traced.iter().filter(|t| t.kind == TraceKind::Parameter).count(), 1);
        assert_eq!(traced.iter().filter(|t| t.kind == TraceKind::Local).count(), 1);
    }

    #[test]
    fn parameters_come_out_in_order_with_their_names_only() {
        let slice = lines(
            "// src/core/flamm/FLAMMLoanSwapLib.sol\n\n    function _plan(FLAMMStore.S storage $, address hook, address assetIn,\n        uint256 amountIn, uint256 supply)\n        private\n        view\n        returns (Plan memory p)\n    {",
        );
        // `$` is left out on purpose: it is on nearly every line and tells a reader
        // nothing they can follow.
        assert_eq!(
            signature_parameters(&slice),
            vec!["hook", "assetIn", "amountIn", "supply"]
        );
    }

    #[test]
    fn a_nested_type_does_not_end_the_parameter_list() {
        let slice = lines("    function f(mapping(uint256 => uint256) storage book, uint256[] memory legs) internal {");
        assert_eq!(signature_parameters(&slice), vec!["book", "legs"]);
    }

    #[test]
    fn an_unnamed_parameter_contributes_nothing_to_follow() {
        let slice = lines("    function f(address, uint256 amount) external {");
        assert_eq!(signature_parameters(&slice), vec!["amount"]);
    }

    /// A declaration of one word is a TYPE, never a name — `returns (bytes32)` was being
    /// read as a variable called `bytes32`, and a user-defined type would have fared the
    /// same.
    #[test]
    fn a_bare_type_is_not_a_name() {
        let slice = lines(
            "    function initcodeHash(address a) internal pure returns (bytes32) {\n        return keccak256(x);\n    }",
        );
        assert_eq!(signature_parameters(&slice), vec!["a"]);
        assert!(named_returns(&slice).is_empty(), "bytes32 is the type, not a name");
    }

    #[test]
    fn a_user_defined_type_without_a_name_is_not_a_name_either() {
        let slice = lines("    function f() internal returns (Plan) {");
        assert!(named_returns(&slice).is_empty());
    }

    #[test]
    fn a_named_return_of_a_user_type_still_counts() {
        let slice = lines("    function f() internal returns (Plan memory p) {");
        assert_eq!(named_returns(&slice), vec!["p"]);
    }

    /// `address payable` is two words and neither is a name.
    #[test]
    fn a_data_location_standing_last_is_not_a_name() {
        let slice = lines("    function f(address payable, bytes32 salt) external {");
        assert_eq!(signature_parameters(&slice), vec!["salt"]);
    }

    #[test]
    fn a_slice_with_no_signature_traces_nothing() {
        assert!(signature_parameters(&lines("        p.feeWad = f.feeWad;")).is_empty());
    }

    /// Past the palette a reader cannot tell the colours apart, so the rest stay plain —
    /// the same rule the connectors use when they run out of hues.
    #[test]
    fn the_list_is_capped_at_the_palette() {
        let slice = lines(
            "    function f(uint a, uint b, uint c, uint d, uint e, uint g, uint h, uint i) internal {",
        );
        assert_eq!(
            signature_parameters(&slice).len(),
            crate::batbelt::silicon::TRACE_COLORS.len()
        );
    }
}

#[cfg(test)]
mod render_name_test {
    use super::*;

    /// The rendered file's LAST extension is what picks the syntax, so the temporary name
    /// a screenshot is written under has to keep `.js` at the end — otherwise Solidity is
    /// highlighted as Rust and every screenshot in the run changes colour.
    #[test]
    fn partial_render_name_keeps_the_extension_last() {
        let name = partial_render_name("fn__src_core_flamm_FLAMMGateLib_sol_222_245.js", 4242);
        assert!(name.ends_with(".js"), "{name} would be highlighted as Rust");
        assert!(name.contains("part4242"), "{name} is not unique to this process");
        assert_ne!(name, "fn__src_core_flamm_FLAMMGateLib_sol_222_245.js");
    }

    /// A name that somehow arrives without the extension still gets one.
    #[test]
    fn partial_render_name_adds_the_extension_when_missing() {
        assert!(partial_render_name("fn_whatever", 7).ends_with(".js"));
    }
}

#[cfg(test)]
mod back_card_test {
    use super::*;

    /// The corner the content does not reach: bottom-right, stacking upward.
    #[test]
    fn the_way_back_goes_in_the_free_corner() {
        let (w, h) = (10_000.0, 4_000.0);
        let content = [(2_000.0, 1_000.0, 3_000.0, 1_500.0)];
        let first = back_card_slot(w, h, &content, 0).expect("the corner is free");
        assert!(first.0 > w / 2.0 && first.1 > h / 2.0, "bottom-right: {first:?}");

        // A second origin stacks above the first, not on top of it.
        let second = back_card_slot(w, h, &content, 1).expect("room for two");
        assert!((first.0 - second.0).abs() < 1.0, "same column");
        assert!(second.1 < first.1 - LINK_CARD_HEIGHT, "clear of the first: {second:?}");
    }

    /// Code in that corner wins: a way back is worth less than the source under it.
    #[test]
    fn content_in_the_corner_refuses_the_card() {
        let (w, h) = (3_000.0, 2_000.0);
        let occupying = [(w - 400.0, h - 300.0, 1_200.0, 800.0)];
        assert!(back_card_slot(w, h, &occupying, 0).is_none());
    }

    /// Stacking cannot climb out of the frame.
    #[test]
    fn stacking_stops_at_the_top() {
        let (w, h) = (3_000.0, 1_200.0);
        assert!(back_card_slot(w, h, &[], 0).is_some());
        assert!(back_card_slot(w, h, &[], 9).is_none());
    }
}

#[cfg(test)]
mod marking_test {
    use super::*;
    use crate::batbelt::evm::metadata::bat_metadata::ExternalUnknownCall;
    use crate::batbelt::evm::types::{EvmContractType, EvmMutability, EvmParam, EvmVisibility};

    fn function(name: &str, params: usize, external: &[(&str, &str)]) -> FunctionMetadata {
        FunctionMetadata {
            metadata_id: format!("f_{name}_{params}"),
            name: name.to_string(),
            contract_name: String::new(),
            visibility: EvmVisibility::Internal,
            mutability: EvmMutability::NonPayable,
            modifiers: Vec::new(),
            params: (0..params)
                .map(|i| EvmParam {
                    name: format!("p{i}"),
                    type_name: "uint256".to_string(),
                    storage_location: None,
                })
                .collect(),
            returns: Vec::new(),
            line: 1,
            end_line: 2,
            is_constructor: false,
            is_stub: false,
            storage_writes: Vec::new(),
            storage_write_sites: Vec::new(),
            unresolved_calls: Vec::new(),
            unknown_external_calls: external
                .iter()
                .map(|(receiver, method)| ExternalUnknownCall {
                    receiver: receiver.to_string(),
                    method: method.to_string(),
                    inferred_type: String::new(),
                })
                .collect(),
            resolved_calls: Vec::new(),
        }
    }

    fn contract(name: &str, using: &[&str], functions: Vec<FunctionMetadata>) -> ContractMetadata {
        ContractMetadata {
            metadata_id: name.to_string(),
            name: name.to_string(),
            using_libraries: using.iter().map(|u| u.to_string()).collect(),
            file_path: format!("./src/{name}.sol"),
            contract_type: EvmContractType::Contract,
            base_contracts: Vec::new(),
            functions,
            state_variables: Vec::new(),
            events: Vec::new(),
            modifiers: Vec::new(),
            line: 1,
            external: false,
        }
    }

    /// `using Address for address;` binds a bare method to a library — and the receiver
    /// becomes that library function's FIRST parameter, so the call site shows one
    /// argument fewer than the function declares.
    #[test]
    fn a_using_library_resolves_with_the_receiver_as_first_parameter() {
        let library = contract("Address", &[], vec![function("functionCall", 3, &[])]);
        let caller = contract("Store", &["Address"], vec![function("pull", 1, &[])]);
        let metadata = EvmBatMetadata {
            contracts: vec![library, caller.clone()],
            ..Default::default()
        };
        let definers: HashMap<String, Vec<String>> =
            HashMap::from([("functionCall".to_string(), vec!["Address".to_string()])]);

        // Two arguments at the call site, three in the declaration.
        let resolved = resolve_call(
            &metadata,
            &caller,
            "functionCall",
            Some(2),
            &AutoDeployOptions::default(),
            &definers,
        );
        assert!(resolved.is_some(), "the using directive should bind the call");
        assert_eq!(resolved.unwrap().0.name, "Address");
    }

    /// An overload delegating to its longer sibling is the same callee BY NAME, so
    /// following only the first went back into itself and reported that a chain reached
    /// nothing. The boundary lives in the three-parameter one.
    #[test]
    fn every_overload_is_walked_when_looking_for_a_boundary() {
        let library = contract(
            "Address",
            &[],
            vec![
                function("functionCall", 2, &[]),
                function("functionCall", 3, &[("assembly", "call")]),
            ],
        );
        let caller = contract("Store", &["Address"], vec![function("pull", 1, &[])]);
        let metadata = EvmBatMetadata {
            contracts: vec![library.clone(), caller.clone()],
            function_dependencies: vec![crate::batbelt::evm::metadata::bat_metadata::FunctionDependency {
                function_metadata_id: "f_pull_1".to_string(),
                callees: vec!["functionCall".to_string()],
            }],
            ..Default::default()
        };
        let definers: HashMap<String, Vec<String>> =
            HashMap::from([("functionCall".to_string(), vec!["Address".to_string()])]);

        let reaches = leads_to_external(
            &metadata,
            &caller,
            &caller.functions[0],
            &AutoDeployOptions::default(),
            &definers,
            &mut HashMap::new(),
            &mut HashSet::new(),
        );
        assert!(reaches, "the boundary is in the overload the first one delegates to");
    }

    /// The frame nearest the boundary owns the mark: a call whose callee is drawn here
    /// keeps the frame clean, and one whose callee is absent carries it.
    #[test]
    fn a_marked_call_survives_only_when_its_callee_is_not_drawn() {
        let mut node = GraphNode {
            id: "C::f".to_string(),
            label: "C.f".to_string(),
            kind: NodeKind::Screenshot,
            file_path: String::new(),
            start_line: 1,
            end_line: 2,
            depth: 0,
            font_size: 32,
            scale: 1.0,
            png_path: String::new(),
            png_width: 0,
            png_height: 0,
            rendered_lines: Vec::new(),
            line_offset: 0,
            writes_storage: false,
            write_lines: Vec::new(),
            external_call_lines: Vec::new(),
            leads_to_write: false,
            write_call_lines: Vec::new(),
            external_call_sites: vec![
                (10, "drawn".to_string(), "C::drawn".to_string()),
                (20, "absent".to_string(), "C::absent".to_string()),
            ],
        };
        let drawn: HashSet<&str> = HashSet::from(["C::drawn"]);
        let kept: Vec<usize> = surviving_external_calls(&node, &drawn)
            .map(|(line, _, _)| *line)
            .collect();
        assert_eq!(kept, vec![20]);

        node.external_call_sites.clear();
        assert_eq!(surviving_external_calls(&node, &drawn).count(), 0);
    }

    /// The band goes on the call, not on a signature that happens to share its name.
    #[test]
    fn the_band_skips_a_signature_that_shares_the_callees_name() {
        let slice: Vec<String> = vec![
            "    function borrow(bytes32 id, uint256 assets, address to) external onlyRouter {"
                .to_string(),
            "        if (to != POOL) revert BadReceiver();".to_string(),
            "        (uint256 borrowed,) = MORPHO.borrow(m, assets, 0, address(this), to);"
                .to_string(),
            "    }".to_string(),
        ];
        assert_eq!(boundary_line_index(&slice, "MORPHO", "borrow"), Some(2));
        // With no receiver to go on, the declaration line is still never the answer.
        assert_eq!(boundary_line_index(&slice, "", "borrow"), Some(2));
        assert_eq!(boundary_line_index(&slice, "MORPHO", "nothingHere"), None);
    }

    /// A call carrying a value block is still a call — `target.call{value: v}(data)` is
    /// the one that moves ether, and missing it put the band nowhere.
    #[test]
    fn a_call_with_a_value_block_is_found_on_its_line() {
        assert!(line_has_call("(bool ok, ) = target.call{value: v}(data);", "call"));
        assert!(line_has_call("let s := call(gas(), token, 0, 0, 0, 0, 0)", "call"));
        assert!(!line_has_call("_callOptionalReturn(token, data);", "call"));
    }
}

#[cfg(test)]
mod natspec_test {
    use super::*;

    /// Writes `body` to a scratch .sol file and returns its path.
    fn sol_fixture(name: &str, body: &str) -> String {
        let dir = std::env::temp_dir().join(format!("bat-cli-natspec-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{name}.sol"));
        std::fs::write(&path, body).unwrap();
        path.to_string_lossy().to_string()
    }

    #[test]
    fn takes_a_run_of_slash_slash_slash_lines() {
        //                   1                2                3               4
        let path = sol_fixture("triple", "contract C {\n    /// @notice a\n    /// @dev b\n    function f() {}\n}\n");
        assert_eq!(natspec_start(&path, 4), 2); // the whole run, not just the last line
    }

    #[test]
    fn takes_a_block_comment_whole() {
        let path = sol_fixture(
            "block",
            "contract C {\n    /**\n     * @notice a\n     */\n    function f() {}\n}\n",
        );
        assert_eq!(natspec_start(&path, 5), 2); // opens at `/**`, four lines up
    }

    #[test]
    fn takes_a_one_line_block() {
        let path = sol_fixture("oneline", "contract C {\n    /** @notice a */\n    function f() {}\n}\n");
        assert_eq!(natspec_start(&path, 3), 2);
    }

    #[test]
    fn leaves_an_ordinary_comment_out() {
        // `//` is a note to the reader, not documentation of the function.
        let path = sol_fixture("plain", "contract C {\n    // just a note\n    function f() {}\n}\n");
        assert_eq!(natspec_start(&path, 3), 3);
    }

    #[test]
    fn a_blank_line_detaches_the_comment() {
        // A comment separated by a blank line belongs to whatever came before it as
        // often as not; pulling it in would put another function's docs on this one.
        let path = sol_fixture("detached", "contract C {\n    /// @notice a\n\n    function f() {}\n}\n");
        assert_eq!(natspec_start(&path, 4), 4);
    }

    #[test]
    fn takes_nothing_when_there_is_nothing() {
        let path = sol_fixture("bare", "contract C {\n    function f() {}\n}\n");
        assert_eq!(natspec_start(&path, 2), 2);
        // Out of range and line 1 are handled without panicking.
        assert_eq!(natspec_start(&path, 1), 1);
        assert_eq!(natspec_start(&path, 999), 999);
        assert_eq!(natspec_start("/no/such/file.sol", 7), 7);
    }

    #[test]
    fn doc_lines_above_is_zero_unless_asked_for() {
        let path = sol_fixture("gated", "contract C {\n    /// @notice a\n    function f() {}\n}\n");
        let mut options = AutoDeployOptions::default();
        assert_eq!(doc_lines_above(&options, &path, 3), 0);
        options.with_documentation = true;
        assert_eq!(doc_lines_above(&options, &path, 3), 1);
    }
}
