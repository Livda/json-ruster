use crate::model::DataNode;

#[derive(Debug, Clone)]
pub struct GraphNode {
    pub id: usize,
    pub parent: Option<usize>,
    pub title: String,
    pub fields: Vec<(String, String)>,
    pub children: Vec<usize>,
}

pub struct Graph {
    pub nodes: Vec<GraphNode>,
    pub root: usize,
}

impl Graph {
    /// Dot-joined path from the root to `id` (e.g. "root.author.name" or
    /// "root.tags[0]"), used to show the user where the selected node sits.
    pub fn path_to(&self, id: usize) -> String {
        let mut titles = Vec::new();
        let mut current = Some(id);
        while let Some(node_id) = current {
            let node = &self.nodes[node_id];
            titles.push(node.title.as_str());
            current = node.parent;
        }
        titles.reverse();

        let mut path = String::new();
        for title in titles {
            if !path.is_empty() && !title.starts_with('[') {
                path.push('.');
            }
            path.push_str(title);
        }
        path
    }
}

pub fn build_graph(data: &DataNode) -> Graph {
    let mut nodes = Vec::new();
    let root = build_node(data, "root".to_string(), None, &mut nodes);
    Graph { nodes, root }
}

fn scalar_repr(node: &DataNode) -> Option<String> {
    match node {
        DataNode::Scalar(s) => Some(s.clone()),
        DataNode::Null => Some("null".to_string()),
        _ => None,
    }
}

fn build_node(
    data: &DataNode,
    title: String,
    parent: Option<usize>,
    nodes: &mut Vec<GraphNode>,
) -> usize {
    let id = nodes.len();
    nodes.push(GraphNode {
        id,
        parent,
        title,
        fields: Vec::new(),
        children: Vec::new(),
    });

    let mut fields = Vec::new();
    let mut children = Vec::new();

    match data {
        DataNode::Object(entries) => {
            for (key, value) in entries {
                if let Some(s) = scalar_repr(value) {
                    fields.push((key.clone(), s));
                } else {
                    children.push(build_node(value, key.clone(), Some(id), nodes));
                }
            }
        }
        DataNode::Array(items) => {
            for (i, value) in items.iter().enumerate() {
                if let Some(s) = scalar_repr(value) {
                    fields.push((i.to_string(), s));
                } else {
                    children.push(build_node(value, format!("[{i}]"), Some(id), nodes));
                }
            }
        }
        DataNode::Scalar(s) => fields.push((String::new(), s.clone())),
        DataNode::Null => fields.push((String::new(), "null".to_string())),
    }

    nodes[id].fields = fields;
    nodes[id].children = children;
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_object_has_no_children() {
        let data = DataNode::Object(vec![
            ("a".into(), DataNode::Scalar("1".into())),
            ("b".into(), DataNode::Scalar("x".into())),
        ]);
        let graph = build_graph(&data);
        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(graph.nodes[graph.root].fields.len(), 2);
    }

    #[test]
    fn nested_object_creates_child_node() {
        let data = DataNode::Object(vec![(
            "author".into(),
            DataNode::Object(vec![("name".into(), DataNode::Scalar("A".into()))]),
        )]);
        let graph = build_graph(&data);
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.nodes[graph.root].children.len(), 1);
    }

    #[test]
    fn path_to_joins_object_and_array_segments() {
        let data = DataNode::Object(vec![
            (
                "author".into(),
                DataNode::Object(vec![("name".into(), DataNode::Scalar("A".into()))]),
            ),
            (
                "tags".into(),
                DataNode::Array(vec![DataNode::Object(vec![(
                    "k".into(),
                    DataNode::Scalar("v".into()),
                )])]),
            ),
        ]);
        let graph = build_graph(&data);
        let author_id = graph.nodes[graph.root].children[0];
        let tags_id = graph.nodes[graph.root].children[1];
        let tag0_id = graph.nodes[tags_id].children[0];

        assert_eq!(graph.path_to(graph.root), "root");
        assert_eq!(graph.path_to(author_id), "root.author");
        assert_eq!(graph.path_to(tag0_id), "root.tags[0]");
    }
}
