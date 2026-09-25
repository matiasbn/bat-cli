//! A frame for a type, when the type is a tree.
//!
//! A struct with no struct fields is a single screenshot: `bat-cli screenshot` drops it on
//! the frame that needed it and there is nothing else to say. A struct whose fields are
//! themselves structs is a different object — `FLAMMSwapLib.Plan` holds a `SwapContext`
//! which holds a `PoolContext` — and drawing only the outer one answers half the question
//! while drawing all of them inline buries the function the frame is about.
//!
//! So it becomes what it is: its own frame, laid out by the same rules a call graph is,
//! with an arrow from each field to the type it names. What the asking frame gets is a
//! card in the type's colours pointing at it, the same shape a branch cut out to its own
//! frame leaves behind.
//!
//! The frame is placed NEXT TO the frame that asked for it — not wherever the board-level
//! allocator's cursor happens to be — because it is read together with it. Miro refuses to
//! create a frame overlapping another (the API answers 500), so free space is found by
//! testing candidate rectangles against every frame the registry knows about, spiralling
//! outward from the host.

use colored::Colorize;
use error_stack::{Report, ResultExt};
use std::collections::{HashMap, HashSet};

use crate::batbelt::evm::metadata::bat_metadata::{AutoDeployedFrame, EvmBatMetadata};
use crate::batbelt::evm::miro::auto_deploy::{
    line_anchor, save_frame_record, ANCHOR_MARKER_SIZE, BOARD_UNITS_PER_PIXEL, PATH_HEADER_LINES,
    SIGNATURE_LINE_INDEX,
};
use crate::batbelt::evm::miro::EvmMiroError;
use crate::batbelt::evm::types::EvmFileItemKind;
use crate::batbelt::miro::client::{ArrowEnd, ConnectorStyle, MiroClient, RelativeAnchor};
use crate::batbelt::miro::layout::{layout_graph, LayoutConfig, LayoutEdge, LayoutNode};
use crate::batbelt::silicon;

type Result<T> = error_stack::Result<T, EvmMiroError>;

/// The colour a type gets, everywhere: the card in the asking frame and the arrows
/// between the fields and the types they name. Nothing else on a board is this colour.
const STRUCT_COLOR: &str = "#a259ff";
const CARD_WIDTH: f64 = 900.0;
const CARD_HEIGHT: f64 = 240.0;

/// One type in the tree: what it is called, where its source is, and its rendered image.
pub struct TypeNode {
    pub label: String,
    /// The lines the screenshot shows, path header included, so a marker can be put at
    /// the end of a given line's text rather than at the image's border.
    pub rendered_lines: Vec<String>,
    /// The source line the image's numbering starts from.
    pub line_offset: usize,
    pub file_path: String,
    pub start: usize,
    pub end: usize,
    pub png_path: String,
    pub png_width: u32,
    pub png_height: u32,
}

/// A field of one type whose declared type is another type in the tree.
pub struct TypeEdge {
    pub from: usize,
    pub to: usize,
    /// Which line of the parent declares it, counted from the start of its screenshot.
    pub line_in_slice: usize,
}

/// The types `label` reaches through its fields, transitively, or `None` when it reaches
/// none — in which case the caller draws it inline as before.
///
/// A field's type is read from the source rather than from the scan, because the scan does
/// not record a struct's field types. The first identifier on the line is the type, with
/// `T[]` and `mapping(K => V)` unwrapped to what they hold; a name is a type when the scan
/// knows a struct or enum by it, preferring one declared in the same file — the usual rule
/// that a name means the nearest declaration, not the first one indexed.
pub fn resolve_tree(
    metadata: &EvmBatMetadata,
    label: &str,
    file_path: &str,
    start: usize,
    end: usize,
) -> Option<(Vec<TypeNode>, Vec<TypeEdge>)> {
    let mut nodes: Vec<TypeNode> = vec![TypeNode {
        label: label.to_string(),
        rendered_lines: Vec::new(),
        line_offset: 0,
        file_path: file_path.to_string(),
        start,
        end,
        png_path: String::new(),
        png_width: 0,
        png_height: 0,
    }];
    let mut edges: Vec<TypeEdge> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    seen.insert(label.to_string());

    let mut queue = vec![0usize];
    while let Some(index) = queue.pop() {
        let (path, from, to) = {
            let node = &nodes[index];
            (node.file_path.clone(), node.start, node.end)
        };
        let lines = crate::batbelt::evm::miro::auto_deploy::read_slice(&path, from, to);
        for (offset, line) in lines.iter().enumerate() {
            let Some(type_name) = field_type(line) else {
                continue;
            };
            let Some(item) = declaration_of(metadata, &type_name, &path) else {
                continue;
            };
            let child_label = format!("{type_name}");
            let target = match nodes.iter().position(|node| node.label == child_label) {
                Some(existing) => existing,
                None => {
                    nodes.push(TypeNode {
                        label: child_label.clone(),
                        rendered_lines: Vec::new(),
                        line_offset: 0,
                        file_path: item.0.clone(),
                        start: item.1,
                        end: item.2,
                        png_path: String::new(),
                        png_width: 0,
                        png_height: 0,
                    });
                    if seen.insert(child_label) {
                        queue.push(nodes.len() - 1);
                    }
                    nodes.len() - 1
                }
            };
            edges.push(TypeEdge {
                from: index,
                to: target,
                // +1 for the path header line the screenshot starts with, +1 for 1-based.
                line_in_slice: offset + 1,
            });
        }
    }

    if edges.is_empty() {
        return None;
    }
    Some((nodes, edges))
}

/// The declared type on a field line, with arrays and mappings unwrapped.
fn field_type(line: &str) -> Option<String> {
    let code = line.split("//").next().unwrap_or(line).trim();
    if code.is_empty() || code.starts_with("struct") || code.starts_with('}') {
        return None;
    }
    // `mapping(address => Position)` — what matters is what it holds.
    let code = match code.split_once("=>") {
        Some((_, value)) => value.trim_start(),
        None => code,
    };
    let first: String = code
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if first.is_empty() {
        return None;
    }
    // A value type is never a tree; skipping them here keeps the lookup cheap and, more
    // to the point, keeps a contract called `Address` from being mistaken for one.
    const VALUE_TYPES: &[&str] = &[
        "uint", "int", "bool", "address", "bytes", "string", "mapping", "struct", "enum",
    ];
    if VALUE_TYPES.iter().any(|value| first.starts_with(value)) {
        return None;
    }
    Some(first)
}

/// Where a struct or enum called `name` is declared, preferring `from_file`.
fn declaration_of(
    metadata: &EvmBatMetadata,
    name: &str,
    from_file: &str,
) -> Option<(String, usize, usize)> {
    let candidates: Vec<_> = metadata
        .file_items
        .iter()
        .filter(|item| {
            item.name == name
                && matches!(item.kind, EvmFileItemKind::Struct | EvmFileItemKind::Enum)
        })
        .collect();
    let chosen = candidates
        .iter()
        .find(|item| item.file_path == from_file)
        .or_else(|| candidates.first())?;
    Some((chosen.file_path.clone(), chosen.line, chosen.end_line))
}

/// Draw the tree into a frame of its own beside `host`, and leave a card on `host`.
///
/// Returns the new frame's URL.
pub async fn draw(
    client: &MiroClient,
    host: &AutoDeployedFrame,
    nodes: Vec<TypeNode>,
    edges: Vec<TypeEdge>,
    font_size: usize,
) -> Result<String> {
    let root_label = nodes[0].label.clone();

    let layout_nodes: Vec<LayoutNode> = nodes
        .iter()
        .map(|node| LayoutNode {
            id: node.label.clone(),
            width: node.png_width as f64 * BOARD_UNITS_PER_PIXEL,
            height: node.png_height as f64 * BOARD_UNITS_PER_PIXEL,
        })
        .collect();
    let geometry = silicon::line_geometry(Some(font_size));
    let layout_edges: Vec<LayoutEdge> = edges
        .iter()
        .map(|edge| LayoutEdge {
            from: nodes[edge.from].label.clone(),
            to: nodes[edge.to].label.clone(),
            from_line_fraction: geometry.line_center_fraction(
                edge.line_in_slice + PATH_HEADER_LINES - 1,
                nodes[edge.from].png_height,
            ),
        })
        .collect();
    let layout = layout_graph(&root_label, &layout_nodes, &layout_edges, LayoutConfig::default());

    let (x, y) = free_rect(host, layout.frame_width, layout.frame_height)?;
    let frame_id = client
        .create_frame(
            &format!("auto: {root_label}"),
            x,
            y,
            layout.frame_width,
            layout.frame_height,
            None,
        )
        .await
        .change_context(EvmMiroError)?;
    let frame_url = client.frame_url(&frame_id);

    let mut image_ids: HashMap<String, String> = HashMap::new();
    for node in &nodes {
        let Some(placed) = layout.node(&node.label) else {
            continue;
        };
        let id = client
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
        image_ids.insert(node.label.clone(), id);
    }

    let mut connector_ids = Vec::new();
    let mut marker_ids = Vec::new();
    for (edge, layout_edge) in edges.iter().zip(layout_edges.iter()) {
        let (Some(_from), Some(to)) = (
            image_ids.get(&nodes[edge.from].label),
            image_ids.get(&nodes[edge.to].label),
        ) else {
            continue;
        };
        let (Some(parent), Some(child)) = (
            layout.node(&nodes[edge.from].label),
            layout.node(&nodes[edge.to].label),
        ) else {
            continue;
        };
        // Both ends land ON a line, not on the middle of a box: it leaves the field that
        // declares the type and arrives at the line that declares it. A reader following
        // `SwapContext sctx;` should land on `struct SwapContext {`, and an arrow into the
        // vertical centre of a tall box points at whatever field happens to be halfway
        // down it.
        let arrival = geometry.line_center_fraction(
            SIGNATURE_LINE_INDEX,
            nodes[edge.to].png_height,
        );

        // Miro clips a connector to the item's border, so an endpoint given as a fraction
        // of an image stops AT the edge — next to the field rather than on it. The way a
        // call graph lands on an exact line is an invisible marker at that point, and a
        // type's field is the same problem: the marker goes just past the end of the
        // field's text, inside the screenshot, and the arrow ends there.
        let parent_node = &nodes[edge.from];
        let anchor = line_anchor(
            (parent.x, parent.y, parent.width, parent.height),
            parent_node.png_width,
            parent_node.png_height,
            font_size,
            &parent_node.rendered_lines,
            parent_node.line_offset,
            edge.line_in_slice.saturating_sub(1) + PATH_HEADER_LINES,
            true,
        );
        let (token_x, token_y, edge_x) = (anchor.token_x, anchor.token_y, anchor.edge_x);

        let token = client
            .create_anchor_marker(&frame_id, token_x, token_y, ANCHOR_MARKER_SIZE)
            .await
            .change_context(EvmMiroError)?;
        marker_ids.push(token.clone());
        let _ = child;

        // A second marker on the box's border, level with the line. Without it the arrow
        // comes in wherever Miro decides — over the code, at the wrong angle — because the
        // only straight path it can be given is between two points that share a y. So the
        // leg that crosses the border is that horizontal stub, and everything outside is
        // one elbow that never touches the screenshot.
        let edge = client
            .create_anchor_marker(&frame_id, edge_x, token_y, ANCHOR_MARKER_SIZE)
            .await
            .change_context(EvmMiroError)?;
        marker_ids.push(edge.clone());
        let mut stub = ConnectorStyle {
            stroke_color: STRUCT_COLOR.to_string(),
            stroke_width: "8".to_string(),
            dashed: false,
            caption: None,
            arrow: ArrowEnd::End,
        };
        stub.arrow = ArrowEnd::End;
        connector_ids.push(
            client
                .create_connector(
                    &edge,
                    RelativeAnchor::new(0.0, 0.5),
                    &token,
                    RelativeAnchor::new(1.0, 0.5),
                    stub,
                )
                .await
                .change_context(EvmMiroError)?,
        );

        let id = client
            .create_connector(
                &edge,
                RelativeAnchor::new(1.0, 0.5),
                to,
                RelativeAnchor::new(0.0, arrival),
                ConnectorStyle {
                    stroke_color: STRUCT_COLOR.to_string(),
                    stroke_width: "8".to_string(),
                    dashed: false,
                    caption: None,
                    // The head is on the stub, at the field; this leg only carries the line
                    // from the type back to the border.
                    arrow: ArrowEnd::None,
                },
            )
            .await
            .change_context(EvmMiroError)?;
        connector_ids.push(id);
    }

    let record = AutoDeployedFrame {
        entry_point: root_label.clone(),
        frame_id: frame_id.clone(),
        frame_url: frame_url.clone(),
        x,
        y,
        width: layout.frame_width,
        height: layout.frame_height,
        images: nodes
            .iter()
            .filter_map(|node| image_ids.get(&node.label).map(|id| (node.label.clone(), id.clone())))
            .collect(),
        image_dims: nodes
            .iter()
            .map(|node| (node.label.clone(), node.png_width, node.png_height))
            .collect(),
        node_positions: nodes
            .iter()
            .filter_map(|node| layout.node(&node.label).map(|p| (node.label.clone(), p.x, p.y)))
            .collect(),
        callee_connectors: Vec::new(),
        link_cards: Vec::new(),
        connector_ids,
        marker_ids,
        border_ids: Vec::new(),
        screenshots: Vec::new(),
        // It belongs to the deployment that asked for it, so `--dependency` reaches it.
        cluster_root: host.cluster_root.clone(),
    };
    save_frame_record(&record)?;
    Ok(frame_url)
}

/// A card on the asking frame, pointing at the type's frame.
pub async fn place_card(
    client: &MiroClient,
    host: &mut AutoDeployedFrame,
    label: &str,
    target_url: &str,
    x: f64,
    y: f64,
) -> Result<String> {
    let id = client
        .create_struct_card(&host.frame_id, label, target_url, x, y, CARD_WIDTH, CARD_HEIGHT)
        .await
        .change_context(EvmMiroError)?;
    host.link_cards.push((label.to_string(), id.clone(), String::new()));
    save_frame_record(host)?;
    Ok(id)
}

pub fn card_size() -> (f64, f64) {
    (CARD_WIDTH, CARD_HEIGHT)
}

/// Board coordinates for a frame of this size, next to `host` and on top of nothing.
///
/// Miro rejects a frame that overlaps another outright, so "next to" has to be checked
/// rather than hoped for. Every frame bat-cli knows about is in the registry with its
/// rectangle, which is enough: to the right of the host first, since that is where the eye
/// goes, then below, then left and above, each ring further out than the last.
fn free_rect(host: &AutoDeployedFrame, width: f64, height: f64) -> Result<(f64, f64)> {
    const GAP: f64 = 600.0;
    let metadata = EvmBatMetadata::read_metadata().change_context(EvmMiroError)?;
    let taken: Vec<(f64, f64, f64, f64)> = metadata
        .miro
        .auto
        .frames
        .iter()
        .map(|frame| (frame.x, frame.y, frame.width, frame.height))
        .collect();
    let overlaps = |x: f64, y: f64| {
        taken.iter().any(|(ox, oy, ow, oh)| {
            (x - ox).abs() * 2.0 < width + ow + GAP && (y - oy).abs() * 2.0 < height + oh + GAP
        })
    };

    for ring in 1..40 {
        let step = ring as f64;
        let candidates = [
            (host.x + (host.width + width) / 2.0 + GAP * step, host.y),
            (host.x, host.y + (host.height + height) / 2.0 + GAP * step),
            (host.x - (host.width + width) / 2.0 - GAP * step, host.y),
            (host.x, host.y - (host.height + height) / 2.0 - GAP * step),
        ];
        for (x, y) in candidates {
            if !overlaps(x, y) {
                return Ok((x, y));
            }
        }
    }
    Err(Report::new(EvmMiroError)
        .attach_printable("no free space near the frame to put the type's frame in")
        .attach(crate::Suggestion(
            "move some frames apart on the board, or deploy this entry point again".to_string(),
        )))
}

pub fn announce(label: &str, url: &str) {
    println!("  {} {} drawn as its own frame", "✓".green(), label.bold());
    println!("  {}", url.blue());
}
