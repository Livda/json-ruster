use crate::graph::{Graph, GraphNode};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy)]
pub struct NodeLayout {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

const VERTICAL_SPACING: f64 = 40.0;
const SIBLING_SPACING: f64 = 40.0;
const LINE_HEIGHT: f64 = 20.0;
const CHAR_WIDTH: f64 = 8.0;
const BOX_PADDING: f64 = 12.0;
const MIN_WIDTH: f64 = 100.0;

/// Max characters shown for a title or "key: value" line before it is
/// truncated with an ellipsis. Keeps box width bounded regardless of how
/// long the underlying data is; the full text stays available as a native
/// tooltip in the renderer.
pub const MAX_FIELD_CHARS: usize = 48;

pub fn truncate_display(s: &str) -> String {
    if s.chars().count() <= MAX_FIELD_CHARS {
        s.to_string()
    } else {
        let head: String = s.chars().take(MAX_FIELD_CHARS.saturating_sub(1)).collect();
        format!("{head}\u{2026}")
    }
}

pub fn field_text(key: &str, value: &str) -> String {
    let full = if key.is_empty() {
        value.to_string()
    } else {
        format!("{key}: {value}")
    };
    truncate_display(&full)
}

/// Simplified Reingold-Tilford style tree layout: leaves are placed left to
/// right via a single running offset, and each internal node is centered
/// over its children. Sharing one running offset across the whole
/// post-order traversal guarantees sibling subtrees never overlap.
///
/// Rows are placed at a dynamic y-offset based on the tallest node seen at
/// each depth so far, rather than a fixed row height: a node with many
/// fields can be taller than a single row, and a fixed height would let the
/// next row overlap it.
///
/// Nodes whose id is in `collapsed` are laid out as leaves: their children
/// are skipped entirely, so the returned map only contains currently
/// visible nodes.
pub fn layout(graph: &Graph, collapsed: &HashSet<usize>) -> HashMap<usize, NodeLayout> {
    let sizes: HashMap<usize, (f64, f64)> = graph
        .nodes
        .iter()
        .map(|node| (node.id, node_size(node)))
        .collect();

    let mut x_by_id: HashMap<usize, f64> = HashMap::new();
    let mut next_x = 0.0f64;
    assign_x(graph, graph.root, collapsed, &sizes, &mut next_x, &mut x_by_id);

    let mut depth_by_id: HashMap<usize, usize> = HashMap::new();
    collect_depths(graph, graph.root, collapsed, 0, &mut depth_by_id);

    let max_depth = depth_by_id.values().copied().max().unwrap_or(0);
    let mut row_height = vec![0.0_f64; max_depth + 1];
    for (&id, &depth) in &depth_by_id {
        row_height[depth] = row_height[depth].max(sizes[&id].1);
    }
    let mut row_y = vec![0.0_f64; max_depth + 1];
    for depth in 1..=max_depth {
        row_y[depth] = row_y[depth - 1] + row_height[depth - 1] + VERTICAL_SPACING;
    }

    depth_by_id
        .into_iter()
        .map(|(id, depth)| {
            let (width, height) = sizes[&id];
            (
                id,
                NodeLayout {
                    x: x_by_id[&id],
                    y: row_y[depth],
                    width,
                    height,
                },
            )
        })
        .collect()
}

fn collect_depths(
    graph: &Graph,
    id: usize,
    collapsed: &HashSet<usize>,
    depth: usize,
    out: &mut HashMap<usize, usize>,
) {
    out.insert(id, depth);
    for &child in children_of(graph, collapsed, id) {
        collect_depths(graph, child, collapsed, depth + 1, out);
    }
}

fn children_of<'a>(graph: &'a Graph, collapsed: &HashSet<usize>, id: usize) -> &'a [usize] {
    if collapsed.contains(&id) {
        &[]
    } else {
        &graph.nodes[id].children
    }
}

fn node_size(node: &GraphNode) -> (f64, f64) {
    let field_lines = node.fields.len().max(1);
    let lines = 1 + field_lines; // title + fields

    let max_chars = node
        .fields
        .iter()
        .map(|(k, v)| field_text(k, v).chars().count())
        .chain(std::iter::once(truncate_display(&node.title).chars().count()))
        .max()
        .unwrap_or(4);

    let width = (max_chars as f64 * CHAR_WIDTH + BOX_PADDING * 2.0).max(MIN_WIDTH);
    let height = lines as f64 * LINE_HEIGHT + BOX_PADDING * 2.0;
    (width, height)
}

fn assign_x(
    graph: &Graph,
    id: usize,
    collapsed: &HashSet<usize>,
    sizes: &HashMap<usize, (f64, f64)>,
    next_x: &mut f64,
    x_by_id: &mut HashMap<usize, f64>,
) {
    let children = children_of(graph, collapsed, id);
    if children.is_empty() {
        let (w, _) = sizes[&id];
        x_by_id.insert(id, *next_x);
        *next_x += w + SIBLING_SPACING;
    } else {
        for &child in children {
            assign_x(graph, child, collapsed, sizes, next_x, x_by_id);
        }
        let first = *children.first().unwrap();
        let last = *children.last().unwrap();
        let (last_w, _) = sizes[&last];
        let center = (x_by_id[&first] + (x_by_id[&last] + last_w)) / 2.0 - sizes[&id].0 / 2.0;
        x_by_id.insert(id, center);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::build_graph;
    use crate::model::DataNode;

    #[test]
    fn field_text_truncates_long_values() {
        let long_value = "a".repeat(200);
        let text = field_text("about", &long_value);
        assert!(text.chars().count() <= MAX_FIELD_CHARS);
        assert!(text.ends_with('\u{2026}'));
    }

    #[test]
    fn node_width_is_bounded_for_long_field_values() {
        let data = DataNode::Object(vec![("about".into(), DataNode::Scalar("a".repeat(2000)))]);
        let graph = build_graph(&data);
        let positions = layout(&graph, &HashSet::new());
        let width = positions[&graph.root].width;
        let max_expected = MAX_FIELD_CHARS as f64 * CHAR_WIDTH + BOX_PADDING * 2.0;
        assert!(width <= max_expected, "width {width} exceeded {max_expected}");
    }

    #[test]
    fn siblings_do_not_overlap() {
        let data = DataNode::Object(vec![
            (
                "a".into(),
                DataNode::Object(vec![("x".into(), DataNode::Scalar("1".into()))]),
            ),
            (
                "b".into(),
                DataNode::Object(vec![("y".into(), DataNode::Scalar("2".into()))]),
            ),
        ]);
        let graph = build_graph(&data);
        let positions = layout(&graph, &HashSet::new());

        let a_id = graph.nodes[graph.root].children[0];
        let b_id = graph.nodes[graph.root].children[1];
        let a = positions[&a_id];
        let b = positions[&b_id];
        assert!(a.x + a.width <= b.x, "expected {a:?} to be left of {b:?}");
    }

    #[test]
    fn rows_never_overlap_vertically() {
        // The root has many fields (tall box); its child row must start
        // below the root's bottom edge, not at some fixed row height.
        let data = DataNode::Object(vec![
            ("a".into(), DataNode::Scalar("1".into())),
            ("b".into(), DataNode::Scalar("2".into())),
            ("c".into(), DataNode::Scalar("3".into())),
            ("d".into(), DataNode::Scalar("4".into())),
            ("e".into(), DataNode::Scalar("5".into())),
            (
                "child".into(),
                DataNode::Object(vec![("x".into(), DataNode::Scalar("1".into()))]),
            ),
        ]);
        let graph = build_graph(&data);
        let positions = layout(&graph, &HashSet::new());

        let root = positions[&graph.root];
        let child_id = graph.nodes[graph.root].children[0];
        let child = positions[&child_id];

        assert_eq!(root.y, 0.0);
        assert!(
            child.y >= root.y + root.height,
            "expected child row {child:?} to start below the root {root:?}"
        );
    }

    #[test]
    fn collapsing_a_node_hides_its_descendants() {
        let data = DataNode::Object(vec![(
            "author".into(),
            DataNode::Object(vec![(
                "address".into(),
                DataNode::Object(vec![("city".into(), DataNode::Scalar("Paris".into()))]),
            )]),
        )]);
        let graph = build_graph(&data);
        let author_id = graph.nodes[graph.root].children[0];

        let mut collapsed = HashSet::new();
        collapsed.insert(author_id);
        let positions = layout(&graph, &collapsed);

        assert!(positions.contains_key(&graph.root));
        assert!(positions.contains_key(&author_id));
        assert_eq!(positions.len(), 2, "grandchildren of a collapsed node must not be laid out");
    }
}
