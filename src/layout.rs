use crate::graph::{Graph, GraphNode};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy)]
pub struct NodeLayout {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Direction the tree grows in. `Vertical` (the default) stacks rows
/// top-to-bottom with siblings spread along x, like the rest of this
/// module's naming assumes; `Horizontal` is the same tree turned 90
/// degrees, stacking columns left-to-right with siblings spread along y.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Vertical,
    Horizontal,
}

impl Orientation {
    pub fn toggle(self) -> Self {
        match self {
            Orientation::Vertical => Orientation::Horizontal,
            Orientation::Horizontal => Orientation::Vertical,
        }
    }
}

/// Identifies a single displayed line within a node: either its title or
/// one of its "key: value" fields. Used as the key for tracking which
/// lines are expanded to show their full (untruncated) text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldRef {
    Title,
    Field(usize),
}

pub const VERTICAL_SPACING: f64 = 40.0;
const SIBLING_SPACING: f64 = 40.0;
pub const LINE_HEIGHT: f64 = 20.0;
const CHAR_WIDTH: f64 = 8.0;
const BOX_PADDING: f64 = 12.0;
const MIN_WIDTH: f64 = 100.0;

/// Max characters shown for a title or "key: value" line before it is
/// truncated. Keeps box width bounded regardless of how long the
/// underlying data is: the renderer appends a clickable `MORE_MARKER` that
/// expands the line in place (wrapped over several lines), and its width
/// is accounted for here too.
pub const MAX_FIELD_CHARS: usize = 48;
pub const MORE_MARKER: &str = " [...]";
pub const LESS_MARKER: &str = " [-]";

/// Truncates `s` to `MAX_FIELD_CHARS`, returning the truncated text and
/// whether truncation happened (so the caller can decide how to surface
/// the omitted part, e.g. a clickable marker).
pub fn truncate_display(s: &str) -> (String, bool) {
    if s.chars().count() <= MAX_FIELD_CHARS {
        (s.to_string(), false)
    } else {
        let head: String = s.chars().take(MAX_FIELD_CHARS).collect();
        (head, true)
    }
}

/// Splits `s` into `MAX_FIELD_CHARS`-wide chunks, used to render an
/// expanded line over several rows without growing the box width.
pub fn wrap_text(s: &str) -> Vec<String> {
    if s.is_empty() {
        return vec![String::new()];
    }
    let chars: Vec<char> = s.chars().collect();
    chars
        .chunks(MAX_FIELD_CHARS)
        .map(|c| c.iter().collect())
        .collect()
}

pub fn field_full_text(key: &str, value: &str) -> String {
    if key.is_empty() {
        value.to_string()
    } else {
        format!("{key}: {value}")
    }
}

fn display_width_chars(text: &str, truncated: bool) -> usize {
    text.chars().count()
        + if truncated {
            MORE_MARKER.chars().count()
        } else {
            0
        }
}

fn line_count(text: &str, expanded: bool) -> usize {
    if expanded {
        wrap_text(text).len().max(1)
    } else {
        1
    }
}

/// Simplified Reingold-Tilford style tree layout: leaves are placed left to
/// right via a single running offset, and each internal node is centered
/// over its children. Sharing one running offset across the whole
/// post-order traversal guarantees sibling subtrees never overlap.
///
/// Rows are placed at a dynamic y-offset based on the tallest node seen at
/// each depth so far, rather than a fixed row height: a node with many
/// fields (or an expanded line) can be taller than a single row, and a
/// fixed height would let the next row overlap it.
///
/// Nodes whose id is in `collapsed` are laid out as leaves: their children
/// are skipped entirely, so the returned map only contains currently
/// visible nodes. `expanded` lists which (node id, field) lines are shown
/// in full (wrapped) rather than truncated, which grows that node's height.
pub fn layout(
    graph: &Graph,
    collapsed: &HashSet<usize>,
    expanded: &HashSet<(usize, FieldRef)>,
    orientation: Orientation,
) -> HashMap<usize, NodeLayout> {
    let sizes: HashMap<usize, (f64, f64)> = graph
        .nodes
        .iter()
        .map(|node| (node.id, node_size(node, expanded)))
        .collect();

    // The tree always grows along a "main" axis (depth) and spreads
    // siblings along a "cross" axis; which screen axis (x or y) plays which
    // role is the only thing `orientation` changes.
    let main_of = |(w, h): (f64, f64)| match orientation {
        Orientation::Vertical => h,
        Orientation::Horizontal => w,
    };
    let cross_of = |(w, h): (f64, f64)| match orientation {
        Orientation::Vertical => w,
        Orientation::Horizontal => h,
    };

    let mut cross_by_id: HashMap<usize, f64> = HashMap::new();
    let mut next_cross = 0.0f64;
    assign_cross(
        graph,
        graph.root,
        collapsed,
        &sizes,
        cross_of,
        &mut next_cross,
        &mut cross_by_id,
    );

    let mut depth_by_id: HashMap<usize, usize> = HashMap::new();
    collect_depths(graph, graph.root, collapsed, 0, &mut depth_by_id);

    let max_depth = depth_by_id.values().copied().max().unwrap_or(0);
    let mut row_main = vec![0.0_f64; max_depth + 1];
    for (&id, &depth) in &depth_by_id {
        row_main[depth] = row_main[depth].max(main_of(sizes[&id]));
    }
    let mut row_offset = vec![0.0_f64; max_depth + 1];
    for depth in 1..=max_depth {
        row_offset[depth] = row_offset[depth - 1] + row_main[depth - 1] + VERTICAL_SPACING;
    }

    depth_by_id
        .into_iter()
        .map(|(id, depth)| {
            let (width, height) = sizes[&id];
            let (x, y) = match orientation {
                Orientation::Vertical => (cross_by_id[&id], row_offset[depth]),
                Orientation::Horizontal => (row_offset[depth], cross_by_id[&id]),
            };
            (
                id,
                NodeLayout {
                    x,
                    y,
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

fn node_size(node: &GraphNode, expanded: &HashSet<(usize, FieldRef)>) -> (f64, f64) {
    let title_lines = line_count(&node.title, expanded.contains(&(node.id, FieldRef::Title)));

    let field_lines: usize = if node.fields.is_empty() {
        1
    } else {
        node.fields
            .iter()
            .enumerate()
            .map(|(i, (k, v))| {
                let is_expanded = expanded.contains(&(node.id, FieldRef::Field(i)));
                line_count(&field_full_text(k, v), is_expanded)
            })
            .sum()
    };

    let lines = title_lines + field_lines;

    let max_chars = node
        .fields
        .iter()
        .map(|(k, v)| {
            let (display, truncated) = truncate_display(&field_full_text(k, v));
            display_width_chars(&display, truncated)
        })
        .chain(std::iter::once({
            let (display, truncated) = truncate_display(&node.title);
            display_width_chars(&display, truncated)
        }))
        .max()
        .unwrap_or(4);

    let width = (max_chars as f64 * CHAR_WIDTH + BOX_PADDING * 2.0).max(MIN_WIDTH);
    let height = lines as f64 * LINE_HEIGHT + BOX_PADDING * 2.0;
    (width, height)
}

fn assign_cross(
    graph: &Graph,
    id: usize,
    collapsed: &HashSet<usize>,
    sizes: &HashMap<usize, (f64, f64)>,
    cross_of: impl Fn((f64, f64)) -> f64 + Copy,
    next_cross: &mut f64,
    cross_by_id: &mut HashMap<usize, f64>,
) {
    let children = children_of(graph, collapsed, id);
    if children.is_empty() {
        let size = cross_of(sizes[&id]);
        cross_by_id.insert(id, *next_cross);
        *next_cross += size + SIBLING_SPACING;
    } else {
        for &child in children {
            assign_cross(
                graph,
                child,
                collapsed,
                sizes,
                cross_of,
                next_cross,
                cross_by_id,
            );
        }
        let first = *children.first().unwrap();
        let last = *children.last().unwrap();
        let last_size = cross_of(sizes[&last]);
        let center = (cross_by_id[&first] + (cross_by_id[&last] + last_size)) / 2.0
            - cross_of(sizes[&id]) / 2.0;
        cross_by_id.insert(id, center);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::build_graph;
    use crate::model::DataNode;

    fn no_collapsed() -> HashSet<usize> {
        HashSet::new()
    }

    fn no_expanded() -> HashSet<(usize, FieldRef)> {
        HashSet::new()
    }

    #[test]
    fn truncate_display_truncates_long_text() {
        let long_value = "a".repeat(200);
        let (text, truncated) = truncate_display(&long_value);
        assert_eq!(text.chars().count(), MAX_FIELD_CHARS);
        assert!(truncated);
    }

    #[test]
    fn truncate_display_reports_no_truncation_for_short_text() {
        let (text, truncated) = truncate_display("a: 1");
        assert_eq!(text, "a: 1");
        assert!(!truncated);
    }

    #[test]
    fn wrap_text_splits_into_max_field_chars_chunks() {
        let long_value = "a".repeat(100);
        let lines = wrap_text(&long_value);
        assert_eq!(lines.len(), 3); // 48 + 48 + 4
        assert!(lines.iter().all(|l| l.chars().count() <= MAX_FIELD_CHARS));
    }

    #[test]
    fn node_width_is_bounded_for_long_field_values() {
        let data = DataNode::Object(vec![("about".into(), DataNode::Scalar("a".repeat(2000)))]);
        let graph = build_graph(&data);
        let positions = layout(
            &graph,
            &no_collapsed(),
            &no_expanded(),
            Orientation::Vertical,
        );
        let width = positions[&graph.root].width;
        let max_chars = MAX_FIELD_CHARS + MORE_MARKER.chars().count();
        let max_expected = max_chars as f64 * CHAR_WIDTH + BOX_PADDING * 2.0;
        assert!(
            width <= max_expected,
            "width {width} exceeded {max_expected}"
        );
    }

    #[test]
    fn expanding_a_field_grows_node_height_without_growing_width() {
        let data = DataNode::Object(vec![("about".into(), DataNode::Scalar("a".repeat(200)))]);
        let graph = build_graph(&data);
        let collapsed_positions = layout(
            &graph,
            &no_collapsed(),
            &no_expanded(),
            Orientation::Vertical,
        );

        let mut expanded = HashSet::new();
        expanded.insert((graph.root, FieldRef::Field(0)));
        let expanded_positions = layout(&graph, &no_collapsed(), &expanded, Orientation::Vertical);

        assert!(expanded_positions[&graph.root].height > collapsed_positions[&graph.root].height);
        assert_eq!(
            expanded_positions[&graph.root].width,
            collapsed_positions[&graph.root].width
        );
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
        let positions = layout(
            &graph,
            &no_collapsed(),
            &no_expanded(),
            Orientation::Vertical,
        );

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
        let positions = layout(
            &graph,
            &no_collapsed(),
            &no_expanded(),
            Orientation::Vertical,
        );

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
        let positions = layout(&graph, &collapsed, &no_expanded(), Orientation::Vertical);

        assert!(positions.contains_key(&graph.root));
        assert!(positions.contains_key(&author_id));
        assert_eq!(
            positions.len(),
            2,
            "grandchildren of a collapsed node must not be laid out"
        );
    }

    #[test]
    fn orientation_toggle_swaps_vertical_and_horizontal() {
        assert_eq!(Orientation::Vertical.toggle(), Orientation::Horizontal);
        assert_eq!(Orientation::Horizontal.toggle(), Orientation::Vertical);
    }

    #[test]
    fn horizontal_orientation_grows_along_x_instead_of_y() {
        let data = DataNode::Object(vec![(
            "child".into(),
            DataNode::Object(vec![("x".into(), DataNode::Scalar("1".into()))]),
        )]);
        let graph = build_graph(&data);
        let child_id = graph.nodes[graph.root].children[0];

        let positions = layout(
            &graph,
            &no_collapsed(),
            &no_expanded(),
            Orientation::Horizontal,
        );
        let root = positions[&graph.root];
        let child = positions[&child_id];

        assert_eq!(root.x, 0.0);
        assert!(
            child.x >= root.x + root.width,
            "expected child column {child:?} to start right of the root {root:?}"
        );
    }
}
