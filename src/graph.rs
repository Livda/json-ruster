use crate::model::DataNode;

#[derive(Debug, Clone)]
pub struct GraphNode {
    pub id: usize,
    pub title: String,
    pub fields: Vec<(String, String)>,
    pub children: Vec<usize>,
}

pub struct Graph {
    pub nodes: Vec<GraphNode>,
    pub root: usize,
}

pub fn build_graph(data: &DataNode) -> Graph {
    let mut nodes = Vec::new();
    let root = build_node(data, "root".to_string(), &mut nodes);
    Graph { nodes, root }
}

fn scalar_repr(node: &DataNode) -> Option<String> {
    match node {
        DataNode::Scalar(s) => Some(s.clone()),
        DataNode::Null => Some("null".to_string()),
        _ => None,
    }
}

fn build_node(data: &DataNode, title: String, nodes: &mut Vec<GraphNode>) -> usize {
    let id = nodes.len();
    nodes.push(GraphNode {
        id,
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
                    children.push(build_node(value, key.clone(), nodes));
                }
            }
        }
        DataNode::Array(items) => {
            for (i, value) in items.iter().enumerate() {
                if let Some(s) = scalar_repr(value) {
                    fields.push((i.to_string(), s));
                } else {
                    children.push(build_node(value, format!("[{i}]"), nodes));
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
}
