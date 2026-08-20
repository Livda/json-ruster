use crate::graph::{Graph, GraphNode};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy)]
pub struct NodeLayout {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

const LEVEL_HEIGHT: f64 = 140.0;
const SIBLING_SPACING: f64 = 40.0;
const LINE_HEIGHT: f64 = 20.0;
const CHAR_WIDTH: f64 = 8.0;
const BOX_PADDING: f64 = 12.0;
const MIN_WIDTH: f64 = 100.0;

/// Simplified Reingold-Tilford style tree layout: leaves are placed left to
/// right via a single running offset, and each internal node is centered
/// over its children. Sharing one running offset across the whole
/// post-order traversal guarantees sibling subtrees never overlap.
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

    let mut result = HashMap::new();
    assign_y(graph, graph.root, collapsed, 0, &sizes, &x_by_id, &mut result);
    result
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
        .map(|(k, v)| if k.is_empty() { v.len() } else { k.len() + v.len() + 2 })
        .chain(std::iter::once(node.title.len()))
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

fn assign_y(
    graph: &Graph,
    id: usize,
    collapsed: &HashSet<usize>,
    depth: usize,
    sizes: &HashMap<usize, (f64, f64)>,
    x_by_id: &HashMap<usize, f64>,
    result: &mut HashMap<usize, NodeLayout>,
) {
    let (width, height) = sizes[&id];
    result.insert(
        id,
        NodeLayout {
            x: x_by_id[&id],
            y: depth as f64 * LEVEL_HEIGHT,
            width,
            height,
        },
    );
    for &child in children_of(graph, collapsed, id) {
        assign_y(graph, child, collapsed, depth + 1, sizes, x_by_id, result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::build_graph;
    use crate::model::DataNode;

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
    fn children_are_one_level_below_parent() {
        let data = DataNode::Object(vec![(
            "author".into(),
            DataNode::Object(vec![("name".into(), DataNode::Scalar("A".into()))]),
        )]);
        let graph = build_graph(&data);
        let positions = layout(&graph, &HashSet::new());
        let child_id = graph.nodes[graph.root].children[0];
        assert_eq!(positions[&child_id].y, LEVEL_HEIGHT);
        assert_eq!(positions[&graph.root].y, 0.0);
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
