use crate::model::DataNode;
use serde_json::Value;

pub fn parse_json(input: &str) -> Result<DataNode, String> {
    let value: Value = serde_json::from_str(input).map_err(|e| e.to_string())?;
    Ok(from_value(value))
}

fn from_value(value: Value) -> DataNode {
    match value {
        Value::Object(map) => {
            DataNode::Object(map.into_iter().map(|(k, v)| (k, from_value(v))).collect())
        }
        Value::Array(arr) => DataNode::Array(arr.into_iter().map(from_value).collect()),
        Value::String(s) => DataNode::Scalar(s),
        Value::Number(n) => DataNode::Scalar(n.to_string()),
        Value::Bool(b) => DataNode::Scalar(b.to_string()),
        Value::Null => DataNode::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_flat_object() {
        let data = parse_json(r#"{"a": 1, "b": "x"}"#).unwrap();
        assert_eq!(
            data,
            DataNode::Object(vec![
                ("a".into(), DataNode::Scalar("1".into())),
                ("b".into(), DataNode::Scalar("x".into())),
            ])
        );
    }

    #[test]
    fn parses_nested_array() {
        let data = parse_json(r#"[1, [2, 3]]"#).unwrap();
        assert_eq!(
            data,
            DataNode::Array(vec![
                DataNode::Scalar("1".into()),
                DataNode::Array(vec![
                    DataNode::Scalar("2".into()),
                    DataNode::Scalar("3".into()),
                ]),
            ])
        );
    }

    #[test]
    fn reports_parse_errors() {
        assert!(parse_json("{not json").is_err());
    }
}
