//! Deterministic graph layout for the automatic Miro deployment.
//!
//! This module is pure: it takes measured boxes plus a call graph and returns
//! coordinates. It never touches the network, the filesystem or the Miro API,
//! so the whole algorithm is unit-testable.
//!
//! Two independent pieces live here:
//!
//! - [`layout_graph`]: places the screenshots of one entry point inside its
//!   frame (Sugiyama-style layering, left → right).
//! - [`ShelfAllocator`]: places the frames themselves on the board, in O(1) per
//!   frame and without ever asking Miro where there is free space.

use std::collections::{HashMap, HashSet, VecDeque};

/// Tunables for [`layout_graph`]. All distances are in board units.
#[derive(Debug, Clone, Copy)]
pub struct LayoutConfig {
    /// Horizontal margin between the frame border and the first layer.
    pub padding_x: f64,
    /// Vertical margin between the frame border and the content.
    pub padding_y: f64,
    /// Extra vertical room reserved at the top for the frame title.
    pub title_band: f64,
    /// Horizontal gap between layers.
    pub gutter_x: f64,
    /// Vertical gap between nodes of the same layer.
    pub gutter_y: f64,
    /// A layer taller than this is split into side-by-side sub-columns.
    pub max_layer_height: f64,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            padding_x: 250.0,
            padding_y: 200.0,
            title_band: 150.0,
            gutter_x: 450.0,
            gutter_y: 120.0,
            max_layer_height: 12_000.0,
        }
    }
}

/// A measured screenshot waiting to be placed.
#[derive(Debug, Clone)]
pub struct LayoutNode {
    pub id: String,
    pub width: f64,
    pub height: f64,
}

/// A caller → callee edge, carrying where in the caller the call happens.
#[derive(Debug, Clone)]
pub struct LayoutEdge {
    pub from: String,
    pub to: String,
    /// Vertical position of the call inside the caller's screenshot, as a
    /// fraction in `[0, 1]`. Used to order callees so that a call made near the
    /// top of the caller draws above one made near the bottom.
    pub from_line_fraction: f64,
}

/// A node with its final position, in coordinates local to the frame
/// (origin at the frame's top-left corner, `x`/`y` = center of the item, which
/// is what the Miro API expects for a child of a frame).
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedNode {
    pub id: String,
    pub layer: usize,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Result of laying out one entry point.
#[derive(Debug, Clone)]
pub struct GraphLayout {
    pub nodes: Vec<PlacedNode>,
    /// Edges that were classified as cycles and should be drawn dashed.
    pub back_edges: Vec<(String, String)>,
    pub bbox_width: f64,
    pub bbox_height: f64,
    pub frame_width: f64,
    pub frame_height: f64,
}

impl GraphLayout {
    pub fn node(&self, id: &str) -> Option<&PlacedNode> {
        self.nodes.iter().find(|n| n.id == id)
    }
}

/// Lay out the dependency graph of a single entry point.
///
/// Layers come from the **longest** path to the root, not the shortest: a helper
/// called from both layer 1 and layer 3 lands on layer 4, which guarantees every
/// edge points forward and no connector ever runs backwards.
pub fn layout_graph(
    root_id: &str,
    nodes: &[LayoutNode],
    edges: &[LayoutEdge],
    config: LayoutConfig,
) -> GraphLayout {
    // A tree gets the tree algorithm: children in a contiguous band, parent
    // centred on them. That is what makes a diagram with no crossing edges,
    // which layering plus barycentre ordering only approximates.
    if is_tree(nodes, edges) {
        return layout_tree(root_id, nodes, edges, config);
    }

    let by_id: HashMap<&str, &LayoutNode> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    let (forward_edges, back_edges) = split_back_edges(root_id, nodes, edges);
    let layers = assign_layers(root_id, nodes, &forward_edges);
    let mut layer_members = group_by_layer(nodes, &layers);
    order_layers(&mut layer_members, &forward_edges);

    // Split any layer that would make the frame absurdly tall.
    let columns: Vec<Vec<Vec<String>>> = layer_members
        .iter()
        .map(|members| split_into_columns(members, &by_id, config))
        .collect();

    // Geometry per layer.
    let mut layer_widths = Vec::with_capacity(columns.len());
    let mut layer_heights = Vec::with_capacity(columns.len());
    for layer_columns in &columns {
        let sub_gutter = config.gutter_x / 2.0;
        let width: f64 = layer_columns
            .iter()
            .map(|column| column_width(column, &by_id))
            .sum::<f64>()
            + sub_gutter * (layer_columns.len().saturating_sub(1)) as f64;
        let height = layer_columns
            .iter()
            .map(|column| column_height(column, &by_id, config))
            .fold(0.0_f64, f64::max);
        layer_widths.push(width);
        layer_heights.push(height);
    }

    let bbox_width = layer_widths.iter().sum::<f64>()
        + config.gutter_x * (columns.len().saturating_sub(1)) as f64;
    let bbox_height = layer_heights.iter().cloned().fold(0.0_f64, f64::max);

    // Place every node.
    let mut placed = Vec::with_capacity(nodes.len());
    let mut layer_x = config.padding_x;
    for (layer_index, layer_columns) in columns.iter().enumerate() {
        let sub_gutter = config.gutter_x / 2.0;
        let mut column_x = layer_x;
        for column in layer_columns {
            let this_column_width = column_width(column, &by_id);
            let this_column_height = column_height(column, &by_id, config);
            // Center the column vertically against the tallest layer.
            let mut y_cursor =
                config.padding_y + config.title_band + (bbox_height - this_column_height) / 2.0;
            for id in column {
                let node = match by_id.get(id.as_str()) {
                    Some(node) => *node,
                    None => continue,
                };
                placed.push(PlacedNode {
                    id: id.clone(),
                    layer: layer_index,
                    x: column_x + node.width / 2.0,
                    y: y_cursor + node.height / 2.0,
                    width: node.width,
                    height: node.height,
                });
                y_cursor += node.height + config.gutter_y;
            }
            column_x += this_column_width + sub_gutter;
        }
        layer_x += layer_widths[layer_index] + config.gutter_x;
    }

    GraphLayout {
        nodes: placed,
        back_edges,
        bbox_width,
        bbox_height,
        frame_width: bbox_width + 2.0 * config.padding_x,
        frame_height: bbox_height + 2.0 * config.padding_y + config.title_band,
    }
}

/// True when every node has at most one parent and there are no cycles.
fn is_tree(nodes: &[LayoutNode], edges: &[LayoutEdge]) -> bool {
    let mut parents: HashMap<&str, usize> = HashMap::new();
    for edge in edges {
        *parents.entry(edge.to.as_str()).or_insert(0) += 1;
    }
    parents.values().all(|count| *count <= 1) && edges.len() + 1 <= nodes.len().max(1)
}

/// Reingold–Tilford style: x from the depth, y from the subtree's own extent.
///
/// Every subtree owns a contiguous vertical band, so no two edges can cross and
/// an arrow never has to travel across a layer it does not belong to.
fn layout_tree(
    root_id: &str,
    nodes: &[LayoutNode],
    edges: &[LayoutEdge],
    config: LayoutConfig,
) -> GraphLayout {
    let by_id: HashMap<&str, &LayoutNode> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    // Siblings follow the order of the calls that produce them, so a callee
    // invoked near the top of its caller is drawn above one invoked lower down
    // and the arrows leave in the order the code reads.
    let mut ordered: HashMap<&str, Vec<(f64, &str)>> = HashMap::new();
    for edge in edges {
        ordered
            .entry(edge.from.as_str())
            .or_default()
            .push((edge.from_line_fraction, edge.to.as_str()));
    }
    let children: HashMap<&str, Vec<&str>> = ordered
        .into_iter()
        .map(|(parent, mut kids)| {
            kids.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            (parent, kids.into_iter().map(|(_, id)| id).collect())
        })
        .collect();

    // Depth of every node, and the widest node per depth, so layers line up.
    let mut depth: HashMap<&str, usize> = HashMap::new();
    let mut order: Vec<&str> = Vec::new();
    let mut stack = vec![(root_id, 0usize)];
    while let Some((id, level)) = stack.pop() {
        if depth.contains_key(id) {
            continue;
        }
        depth.insert(id, level);
        order.push(id);
        for child in children.get(id).cloned().unwrap_or_default().iter().rev() {
            stack.push((child, level + 1));
        }
    }
    // Nodes unreachable from the root still need a place.
    for node in nodes {
        if !depth.contains_key(node.id.as_str()) {
            depth.insert(node.id.as_str(), 0);
            order.push(node.id.as_str());
        }
    }

    let layer_count = depth.values().copied().max().unwrap_or(0) + 1;
    let mut layer_width = vec![0.0_f64; layer_count];
    for node in nodes {
        let level = depth[node.id.as_str()];
        layer_width[level] = layer_width[level].max(node.width);
    }
    let mut layer_x = vec![0.0_f64; layer_count];
    let mut cursor = config.padding_x;
    for (level, width) in layer_width.iter().enumerate() {
        layer_x[level] = cursor;
        cursor += width + config.gutter_x;
    }

    // Post-order: a leaf takes its own height, a parent spans its children.
    let mut extent: HashMap<&str, f64> = HashMap::new();
    for id in order.iter().rev() {
        let own = by_id.get(id).map(|n| n.height).unwrap_or(0.0);
        let kids = children.get(id).cloned().unwrap_or_default();
        if kids.is_empty() {
            extent.insert(id, own);
            continue;
        }
        let span: f64 = kids.iter().map(|kid| extent.get(kid).copied().unwrap_or(0.0)).sum::<f64>()
            + config.gutter_y * (kids.len().saturating_sub(1)) as f64;
        extent.insert(id, span.max(own));
    }

    // Pre-order: hand each subtree its band, centre the parent inside it.
    let top = config.padding_y + config.title_band;
    let mut placed: Vec<PlacedNode> = Vec::new();
    let mut bands = vec![(root_id, top)];
    while let Some((id, band_top)) = bands.pop() {
        let Some(node) = by_id.get(id) else { continue };
        let band = extent.get(id).copied().unwrap_or(node.height);
        let level = depth[id];

        placed.push(PlacedNode {
            id: id.to_string(),
            layer: level,
            x: layer_x[level] + node.width / 2.0,
            y: band_top + band / 2.0,
            width: node.width,
            height: node.height,
        });

        let kids = children.get(id).cloned().unwrap_or_default();
        let mut child_top = band_top;
        let mut queued = Vec::new();
        for kid in kids {
            queued.push((kid, child_top));
            child_top += extent.get(kid).copied().unwrap_or(0.0) + config.gutter_y;
        }
        for entry in queued.into_iter().rev() {
            bands.push(entry);
        }
    }

    let bbox_width = layer_width.iter().sum::<f64>()
        + config.gutter_x * (layer_count.saturating_sub(1)) as f64;
    let bbox_height = extent.get(root_id).copied().unwrap_or(0.0);

    GraphLayout {
        nodes: placed,
        back_edges: Vec::new(),
        bbox_width,
        bbox_height,
        frame_width: bbox_width + 2.0 * config.padding_x,
        frame_height: bbox_height + 2.0 * config.padding_y + config.title_band,
    }
}

fn column_width(column: &[String], by_id: &HashMap<&str, &LayoutNode>) -> f64 {
    column
        .iter()
        .filter_map(|id| by_id.get(id.as_str()))
        .map(|n| n.width)
        .fold(0.0_f64, f64::max)
}

fn column_height(
    column: &[String],
    by_id: &HashMap<&str, &LayoutNode>,
    config: LayoutConfig,
) -> f64 {
    let sum: f64 = column
        .iter()
        .filter_map(|id| by_id.get(id.as_str()))
        .map(|n| n.height)
        .sum();
    sum + config.gutter_y * (column.len().saturating_sub(1)) as f64
}

/// Break a layer into side-by-side columns when stacking it in one column would
/// exceed `max_layer_height`.
fn split_into_columns(
    members: &[String],
    by_id: &HashMap<&str, &LayoutNode>,
    config: LayoutConfig,
) -> Vec<Vec<String>> {
    let total = column_height(members, by_id, config);
    if total <= config.max_layer_height || members.len() < 2 {
        return vec![members.to_vec()];
    }
    let column_count = (total / config.max_layer_height).ceil() as usize;
    let target = total / column_count as f64;

    let mut columns: Vec<Vec<String>> = Vec::new();
    let mut current: Vec<String> = Vec::new();
    let mut current_height = 0.0;
    for id in members {
        let height = by_id.get(id.as_str()).map(|n| n.height).unwrap_or(0.0);
        if !current.is_empty()
            && current_height + height > target
            && columns.len() + 1 < column_count
        {
            columns.push(std::mem::take(&mut current));
            current_height = 0.0;
        }
        current.push(id.clone());
        current_height += height + config.gutter_y;
    }
    if !current.is_empty() {
        columns.push(current);
    }
    columns
}

/// Classify edges into forward edges and back edges (cycles), using a DFS from
/// the root. Back edges are excluded from the layering so the graph is a DAG.
fn split_back_edges(
    root_id: &str,
    nodes: &[LayoutNode],
    edges: &[LayoutEdge],
) -> (Vec<LayoutEdge>, Vec<(String, String)>) {
    let mut adjacency: HashMap<&str, Vec<usize>> = HashMap::new();
    for (index, edge) in edges.iter().enumerate() {
        adjacency.entry(edge.from.as_str()).or_default().push(index);
    }

    let mut back: HashSet<usize> = HashSet::new();
    let mut state: HashMap<&str, u8> = HashMap::new(); // 0 unseen, 1 on stack, 2 done

    // Iterative DFS so a deep graph cannot blow the stack.
    let mut roots: Vec<&str> = vec![root_id];
    roots.extend(nodes.iter().map(|n| n.id.as_str()));
    for start in roots {
        if state.get(start).copied().unwrap_or(0) != 0 {
            continue;
        }
        let mut stack: Vec<(&str, usize)> = vec![(start, 0)];
        state.insert(start, 1);
        while let Some((node, cursor)) = stack.pop() {
            let outgoing = adjacency.get(node).cloned().unwrap_or_default();
            if cursor < outgoing.len() {
                stack.push((node, cursor + 1));
                let edge_index = outgoing[cursor];
                let target = edges[edge_index].to.as_str();
                match state.get(target).copied().unwrap_or(0) {
                    1 => {
                        back.insert(edge_index);
                    }
                    0 => {
                        state.insert(target, 1);
                        stack.push((target, 0));
                    }
                    _ => {}
                }
            } else {
                state.insert(node, 2);
            }
        }
    }

    let mut forward = Vec::new();
    let mut back_pairs = Vec::new();
    for (index, edge) in edges.iter().enumerate() {
        if back.contains(&index) {
            back_pairs.push((edge.from.clone(), edge.to.clone()));
        } else {
            forward.push(edge.clone());
        }
    }
    (forward, back_pairs)
}

/// Longest-path layering over the acyclic edge set.
fn assign_layers(
    root_id: &str,
    nodes: &[LayoutNode],
    edges: &[LayoutEdge],
) -> HashMap<String, usize> {
    let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    for node in nodes {
        in_degree.entry(node.id.as_str()).or_insert(0);
    }
    for edge in edges {
        adjacency
            .entry(edge.from.as_str())
            .or_default()
            .push(edge.to.as_str());
        *in_degree.entry(edge.to.as_str()).or_insert(0) += 1;
    }

    // Kahn topological order.
    let mut queue: VecDeque<&str> = VecDeque::new();
    if in_degree.get(root_id).copied().unwrap_or(0) == 0 {
        queue.push_back(root_id);
    }
    for node in nodes {
        if node.id != root_id && in_degree.get(node.id.as_str()).copied().unwrap_or(0) == 0 {
            queue.push_back(node.id.as_str());
        }
    }

    let mut layers: HashMap<String, usize> = HashMap::new();
    layers.insert(root_id.to_string(), 0);
    let mut remaining = in_degree.clone();
    let mut order: Vec<&str> = Vec::new();
    while let Some(node) = queue.pop_front() {
        order.push(node);
        for &next in adjacency.get(node).unwrap_or(&Vec::new()) {
            let entry = remaining.entry(next).or_insert(0);
            *entry = entry.saturating_sub(1);
            if *entry == 0 {
                queue.push_back(next);
            }
        }
    }

    for node in &order {
        let current = layers.get(*node).copied().unwrap_or(0);
        for &next in adjacency.get(*node).unwrap_or(&Vec::new()) {
            let candidate = current + 1;
            let entry = layers.entry(next.to_string()).or_insert(candidate);
            if *entry < candidate {
                *entry = candidate;
            }
        }
    }

    for node in nodes {
        layers.entry(node.id.clone()).or_insert(0);
    }
    layers
}

fn group_by_layer(nodes: &[LayoutNode], layers: &HashMap<String, usize>) -> Vec<Vec<String>> {
    let depth = layers.values().copied().max().unwrap_or(0);
    let mut grouped: Vec<Vec<String>> = vec![Vec::new(); depth + 1];
    for node in nodes {
        let layer = layers.get(&node.id).copied().unwrap_or(0);
        grouped[layer].push(node.id.clone());
    }
    grouped
}

/// Barycenter ordering, weighted by where the call sits inside the caller.
///
/// A callee invoked on the third line of its caller is pulled above one invoked
/// on the twelfth, so connectors stay close to horizontal and rarely cross.
fn order_layers(layers: &mut [Vec<String>], edges: &[LayoutEdge]) {
    // An odd number so the **last** sweep is the downward one: the downward pass
    // is the one that honours the call-line order, and an upward pass would
    // undo it by re-sorting on the successors' positions.
    const SWEEPS: usize = 5;

    let mut incoming: HashMap<&str, Vec<&LayoutEdge>> = HashMap::new();
    let mut outgoing: HashMap<&str, Vec<&LayoutEdge>> = HashMap::new();
    for edge in edges {
        incoming.entry(edge.to.as_str()).or_default().push(edge);
        outgoing.entry(edge.from.as_str()).or_default().push(edge);
    }

    for sweep in 0..SWEEPS {
        let downward = sweep % 2 == 0;
        if downward {
            for index in 1..layers.len() {
                let previous: HashMap<String, usize> = layers[index - 1]
                    .iter()
                    .enumerate()
                    .map(|(position, id)| (id.clone(), position))
                    .collect();
                sort_layer(&mut layers[index], |id| {
                    let sources = incoming.get(id)?;
                    let keys: Vec<f64> = sources
                        .iter()
                        .filter_map(|edge| {
                            previous
                                .get(&edge.from)
                                .map(|position| *position as f64 + edge.from_line_fraction)
                        })
                        .collect();
                    mean(&keys)
                });
            }
        } else {
            for index in (0..layers.len().saturating_sub(1)).rev() {
                let next: HashMap<String, usize> = layers[index + 1]
                    .iter()
                    .enumerate()
                    .map(|(position, id)| (id.clone(), position))
                    .collect();
                sort_layer(&mut layers[index], |id| {
                    let targets = outgoing.get(id)?;
                    let keys: Vec<f64> = targets
                        .iter()
                        .filter_map(|edge| {
                            next.get(&edge.to).map(|position| *position as f64)
                        })
                        .collect();
                    mean(&keys)
                });
            }
        }
    }
}

fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    Some(values.iter().sum::<f64>() / values.len() as f64)
}

/// Stable sort by an optional key; nodes without a key keep their relative order
/// at the end, so the layout stays deterministic run to run.
fn sort_layer<F>(layer: &mut [String], key_of: F)
where
    F: Fn(&str) -> Option<f64>,
{
    let mut keyed: Vec<(Option<f64>, String)> = layer
        .iter()
        .map(|id| (key_of(id.as_str()), id.clone()))
        .collect();
    keyed.sort_by(|a, b| match (a.0, b.0) {
        (Some(left), Some(right)) => left.partial_cmp(&right).unwrap_or(std::cmp::Ordering::Equal),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    for (slot, (_, id)) in layer.iter_mut().zip(keyed.into_iter()) {
        *slot = id;
    }
}

// ---------------------------------------------------------------------------
// Board-level placement
// ---------------------------------------------------------------------------

/// Shelf packing allocator for frames.
///
/// Miro's REST API has no "find me free space" endpoint (`findEmptySpace` only
/// exists in the Web SDK), and testing a new frame for collisions against every
/// frame on the board does not scale to hundreds of entry points. Instead we
/// reserve a region of the board and hand out positions from a running cursor:
/// O(1) per frame, no API calls, and overlaps are impossible by construction.
///
/// The state is meant to be persisted in the project metadata so incremental
/// runs keep appending instead of rescanning the board.
#[derive(Debug, Clone, PartialEq)]
pub struct ShelfAllocator {
    /// Top-left corner of the reserved region, in board coordinates.
    pub origin_x: f64,
    pub origin_y: f64,
    /// Cursor inside the region.
    pub cursor_x: f64,
    pub cursor_y: f64,
    /// Height of the tallest frame in the row being filled.
    pub row_height: f64,
    /// A row wraps once it would grow past this width.
    pub row_max_width: f64,
    /// Gap left between frames, horizontally and between rows.
    pub gutter: f64,
}

impl ShelfAllocator {
    /// Default row width: the spiritual successor of the old fixed
    /// `MIRO_BOARD_COLUMNS = 5` grid — five wide frames per row, but frames of
    /// any size are handled without wasting space.
    pub const DEFAULT_ROW_MAX_WIDTH: f64 = 60_000.0;
    pub const DEFAULT_GUTTER: f64 = 1_000.0;

    pub fn new(origin_x: f64, origin_y: f64) -> Self {
        Self {
            origin_x,
            origin_y,
            cursor_x: 0.0,
            cursor_y: 0.0,
            row_height: 0.0,
            row_max_width: Self::DEFAULT_ROW_MAX_WIDTH,
            gutter: Self::DEFAULT_GUTTER,
        }
    }

    /// Reserve room for a frame and return its **center** in board coordinates,
    /// which is what the Miro API expects.
    pub fn place(&mut self, width: f64, height: f64) -> (f64, f64) {
        if self.cursor_x > 0.0 && self.cursor_x + width > self.row_max_width {
            self.cursor_x = 0.0;
            self.cursor_y += self.row_height + self.gutter;
            self.row_height = 0.0;
        }
        let x = self.origin_x + self.cursor_x + width / 2.0;
        let y = self.origin_y + self.cursor_y + height / 2.0;
        self.cursor_x += width + self.gutter;
        self.row_height = self.row_height.max(height);
        (x, y)
    }
}

#[cfg(test)]
mod layout_test {
    use super::*;

    fn node(id: &str, width: f64, height: f64) -> LayoutNode {
        LayoutNode {
            id: id.to_string(),
            width,
            height,
        }
    }

    fn edge(from: &str, to: &str, fraction: f64) -> LayoutEdge {
        LayoutEdge {
            from: from.to_string(),
            to: to.to_string(),
            from_line_fraction: fraction,
        }
    }

    fn layer_of(layout: &GraphLayout, id: &str) -> usize {
        layout.node(id).expect("node missing").layer
    }

    /// A helper reachable at depth 1 and at depth 2 must land on the deeper
    /// layer, otherwise a connector would point backwards.
    #[test]
    fn test_diamond_uses_longest_path() {
        let nodes = vec![
            node("root", 1000.0, 400.0),
            node("a", 1000.0, 400.0),
            node("b", 1000.0, 400.0),
            node("shared", 1000.0, 400.0),
        ];
        let edges = vec![
            edge("root", "a", 0.2),
            edge("root", "shared", 0.5),
            edge("a", "b", 0.3),
            edge("b", "shared", 0.4),
        ];
        let layout = layout_graph("root", &nodes, &edges, LayoutConfig::default());

        assert_eq!(layer_of(&layout, "root"), 0);
        assert_eq!(layer_of(&layout, "a"), 1);
        assert_eq!(layer_of(&layout, "b"), 2);
        assert_eq!(
            layer_of(&layout, "shared"),
            3,
            "shared must sink to the deepest caller"
        );

        for e in &edges {
            assert!(
                layer_of(&layout, &e.to) > layer_of(&layout, &e.from),
                "edge {} -> {} points backwards",
                e.from,
                e.to
            );
        }
    }

    #[test]
    fn test_cycle_is_reported_as_back_edge_and_does_not_hang() {
        let nodes = vec![
            node("root", 800.0, 300.0),
            node("a", 800.0, 300.0),
            node("b", 800.0, 300.0),
        ];
        let edges = vec![
            edge("root", "a", 0.1),
            edge("a", "b", 0.2),
            edge("b", "a", 0.9),
        ];
        let layout = layout_graph("root", &nodes, &edges, LayoutConfig::default());

        assert_eq!(layout.back_edges, vec![("b".to_string(), "a".to_string())]);
        assert_eq!(layer_of(&layout, "a"), 1);
        assert_eq!(layer_of(&layout, "b"), 2);
    }

    #[test]
    fn test_no_two_nodes_overlap() {
        let mut nodes = vec![node("root", 1800.0, 900.0)];
        let mut edges = Vec::new();
        for i in 0..7 {
            let id = format!("dep{i}");
            nodes.push(node(&id, 1200.0, 300.0 + 90.0 * i as f64));
            edges.push(edge("root", &id, i as f64 / 7.0));
        }
        for i in 0..4 {
            let id = format!("leaf{i}");
            nodes.push(node(&id, 1000.0, 260.0));
            edges.push(edge("dep0", &id, i as f64 / 4.0));
        }
        let layout = layout_graph("root", &nodes, &edges, LayoutConfig::default());

        for (i, a) in layout.nodes.iter().enumerate() {
            for b in layout.nodes.iter().skip(i + 1) {
                let overlap_x = (a.x - b.x).abs() < (a.width + b.width) / 2.0;
                let overlap_y = (a.y - b.y).abs() < (a.height + b.height) / 2.0;
                assert!(
                    !(overlap_x && overlap_y),
                    "{} and {} overlap",
                    a.id,
                    b.id
                );
            }
        }
    }

    #[test]
    fn test_every_node_fits_inside_the_frame() {
        let nodes = vec![
            node("root", 1800.0, 900.0),
            node("a", 1500.0, 1200.0),
            node("b", 1200.0, 400.0),
        ];
        let edges = vec![edge("root", "a", 0.3), edge("a", "b", 0.6)];
        let config = LayoutConfig::default();
        let layout = layout_graph("root", &nodes, &edges, config);

        for placed in &layout.nodes {
            assert!(
                placed.x - placed.width / 2.0 >= 0.0
                    && placed.x + placed.width / 2.0 <= layout.frame_width,
                "{} sticks out horizontally",
                placed.id
            );
            assert!(
                placed.y - placed.height / 2.0 >= 0.0
                    && placed.y + placed.height / 2.0 <= layout.frame_height,
                "{} sticks out vertically",
                placed.id
            );
        }
        // Nothing may intrude into the title band.
        for placed in &layout.nodes {
            assert!(placed.y - placed.height / 2.0 >= config.title_band);
        }
    }

    #[test]
    fn test_call_line_order_drives_vertical_order() {
        let nodes = vec![
            node("root", 1400.0, 900.0),
            node("called_last", 1000.0, 300.0),
            node("called_first", 1000.0, 300.0),
        ];
        // Declared in the wrong order on purpose: the fractions must win.
        let edges = vec![
            edge("root", "called_last", 0.90),
            edge("root", "called_first", 0.05),
        ];
        let layout = layout_graph("root", &nodes, &edges, LayoutConfig::default());

        let first = layout.node("called_first").unwrap();
        let last = layout.node("called_last").unwrap();
        assert!(
            first.y < last.y,
            "the callee invoked earlier in the body must be drawn above"
        );
    }

    /// Column splitting belongs to the layered algorithm, which only runs for
    /// graphs that are not trees — a tree is laid out as a tree, where one tall
    /// band is the honest shape and splitting it would create crossings. The
    /// extra shared node here is what makes this a DAG.
    #[test]
    fn test_tall_layer_is_split_into_columns() {
        let mut nodes = vec![node("root", 1200.0, 400.0), node("shared", 900.0, 200.0)];
        let mut edges = Vec::new();
        for i in 0..20 {
            let id = format!("dep{i}");
            nodes.push(node(&id, 1000.0, 1500.0));
            edges.push(edge("root", &id, i as f64 / 20.0));
            edges.push(edge(&id, "shared", 0.5));
        }
        let config = LayoutConfig {
            max_layer_height: 12_000.0,
            ..LayoutConfig::default()
        };
        let layout = layout_graph("root", &nodes, &edges, config);

        // 20 * 1500 + gutters ≈ 32k, so it must be broken up rather than
        // producing a 32k-tall frame.
        assert!(
            layout.frame_height < 20_000.0,
            "frame height {} was not reduced by column splitting",
            layout.frame_height
        );
        for (i, a) in layout.nodes.iter().enumerate() {
            for b in layout.nodes.iter().skip(i + 1) {
                let overlap_x = (a.x - b.x).abs() < (a.width + b.width) / 2.0;
                let overlap_y = (a.y - b.y).abs() < (a.height + b.height) / 2.0;
                assert!(!(overlap_x && overlap_y), "{} and {} overlap", a.id, b.id);
            }
        }
    }

    #[test]
    fn test_layout_is_deterministic() {
        let nodes = vec![
            node("root", 1800.0, 900.0),
            node("a", 1200.0, 500.0),
            node("b", 1200.0, 700.0),
            node("c", 1200.0, 300.0),
        ];
        let edges = vec![
            edge("root", "a", 0.1),
            edge("root", "b", 0.5),
            edge("a", "c", 0.4),
        ];
        let first = layout_graph("root", &nodes, &edges, LayoutConfig::default());
        let second = layout_graph("root", &nodes, &edges, LayoutConfig::default());
        assert_eq!(first.nodes, second.nodes);
    }

    #[test]
    fn test_shelf_allocator_never_overlaps_and_wraps_rows() {
        let mut allocator = ShelfAllocator::new(0.0, 100_000.0);
        allocator.row_max_width = 20_000.0;

        // Deliberately uneven sizes, the case a uniform grid handles badly.
        let sizes: Vec<(f64, f64)> = (0..40)
            .map(|i| {
                let width = 3_000.0 + (i % 7) as f64 * 2_500.0;
                let height = 1_500.0 + (i % 5) as f64 * 1_200.0;
                (width, height)
            })
            .collect();

        let mut rects = Vec::new();
        for (width, height) in &sizes {
            let (x, y) = allocator.place(*width, *height);
            rects.push((x, y, *width, *height));
        }

        for (i, a) in rects.iter().enumerate() {
            for b in rects.iter().skip(i + 1) {
                let overlap_x = (a.0 - b.0).abs() < (a.2 + b.2) / 2.0;
                let overlap_y = (a.1 - b.1).abs() < (a.3 + b.3) / 2.0;
                assert!(!(overlap_x && overlap_y), "frames {i} overlap");
            }
        }

        // Everything stays inside the reserved region.
        for (x, _y, width, _height) in &rects {
            assert!(x - width / 2.0 >= 0.0);
        }
    }

    #[test]
    fn test_shelf_allocator_state_round_trips() {
        let mut allocator = ShelfAllocator::new(0.0, 0.0);
        allocator.place(5_000.0, 2_000.0);
        let saved = allocator.clone();

        // Resuming from persisted state must continue where it left off.
        let mut resumed = saved.clone();
        let (x_a, y_a) = allocator.place(4_000.0, 1_000.0);
        let (x_b, y_b) = resumed.place(4_000.0, 1_000.0);
        assert_eq!((x_a, y_a), (x_b, y_b));
    }
}

#[cfg(test)]
mod tree_layout_test {
    use super::*;

    fn node(id: &str, width: f64, height: f64) -> LayoutNode {
        LayoutNode {
            id: id.to_string(),
            width,
            height,
        }
    }

    fn edge(from: &str, to: &str) -> LayoutEdge {
        LayoutEdge {
            from: from.to_string(),
            to: to.to_string(),
            from_line_fraction: 0.5,
        }
    }

    /// Two edges cross when one starts above the other and ends below it.
    fn crossings(layout: &GraphLayout, edges: &[LayoutEdge]) -> usize {
        let segment = |edge: &LayoutEdge| {
            let from = layout.node(&edge.from)?;
            let to = layout.node(&edge.to)?;
            Some((from.y, to.y, from.layer))
        };
        let segments: Vec<_> = edges.iter().filter_map(segment).collect();
        let mut count = 0;
        for (index, a) in segments.iter().enumerate() {
            for b in segments.iter().skip(index + 1) {
                if a.2 != b.2 {
                    continue; // only siblings' edges share a corridor
                }
                if (a.0 - b.0) * (a.1 - b.1) < 0.0 {
                    count += 1;
                }
            }
        }
        count
    }

    /// The shape a deployment actually produces: the same helper appears once
    /// per caller instead of being shared.
    #[test]
    fn test_tree_layout_has_no_crossings() {
        let mut nodes = vec![node("root", 1600.0, 700.0)];
        let mut edges = Vec::new();
        for branch in 0..4 {
            let parent = format!("dep{branch}");
            nodes.push(node(&parent, 1200.0, 400.0));
            edges.push(edge("root", &parent));
            for leaf in 0..3 {
                let child = format!("leaf{branch}_{leaf}");
                nodes.push(node(&child, 1000.0, 250.0));
                edges.push(edge(&parent, &child));
            }
        }
        let layout = layout_graph("root", &nodes, &edges, LayoutConfig::default());

        assert_eq!(crossings(&layout, &edges), 0);
        for (i, a) in layout.nodes.iter().enumerate() {
            for b in layout.nodes.iter().skip(i + 1) {
                let overlap_x = (a.x - b.x).abs() < (a.width + b.width) / 2.0;
                let overlap_y = (a.y - b.y).abs() < (a.height + b.height) / 2.0;
                assert!(!(overlap_x && overlap_y), "{} and {} overlap", a.id, b.id);
            }
        }
    }

    /// A parent sits opposite the middle of the band its children occupy, so the
    /// arrows leave it fanning out rather than doubling back.
    #[test]
    fn test_parent_is_centred_on_its_children() {
        let nodes = vec![
            node("root", 1200.0, 300.0),
            node("a", 1000.0, 200.0),
            node("b", 1000.0, 600.0),
            node("c", 1000.0, 200.0),
        ];
        let edges = vec![edge("root", "a"), edge("root", "b"), edge("root", "c")];
        let layout = layout_graph("root", &nodes, &edges, LayoutConfig::default());

        let root = layout.node("root").unwrap();
        let first = layout.node("a").unwrap();
        let last = layout.node("c").unwrap();
        let middle = (first.y + last.y) / 2.0;
        assert!(
            (root.y - middle).abs() < 1.0,
            "root at {} but its children span a midpoint of {middle}",
            root.y
        );
    }

    /// Repeating a shared helper is the whole point: each copy is its own node.
    #[test]
    fn test_repeated_helper_gets_its_own_box() {
        let nodes = vec![
            node("root", 1200.0, 300.0),
            node("a", 1000.0, 200.0),
            node("b", 1000.0, 200.0),
            node("helper#1", 900.0, 150.0),
            node("helper#2", 900.0, 150.0),
        ];
        let edges = vec![
            edge("root", "a"),
            edge("root", "b"),
            edge("a", "helper#1"),
            edge("b", "helper#2"),
        ];
        let layout = layout_graph("root", &nodes, &edges, LayoutConfig::default());

        let first = layout.node("helper#1").unwrap();
        let second = layout.node("helper#2").unwrap();
        assert_ne!(first.y, second.y, "the two copies must not sit on top of each other");
        assert_eq!(first.layer, second.layer);
        assert_eq!(crossings(&layout, &edges), 0);
    }
}
