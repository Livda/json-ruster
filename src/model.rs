#[derive(Debug, Clone, PartialEq)]
pub enum DataNode {
    Object(Vec<(String, DataNode)>),
    Array(Vec<DataNode>),
    Scalar(String),
    Null,
}
