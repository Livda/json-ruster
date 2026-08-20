use crate::model::DataNode;
use serde_yaml::Value;

pub const SAMPLE: &str = r#"name: json-ruster
version: 0.1.0
tags:
  - json
  - rust
  - wasm
author:
  name: John
  active: true
  address:
    city: Paris
"#;

pub fn parse_yaml(input: &str) -> Result<DataNode, String> {
    let value: Value = serde_yaml::from_str(input).map_err(|e| e.to_string())?;
    Ok(from_value(value))
}

fn from_value(value: Value) -> DataNode {
    match value {
        Value::Mapping(map) => DataNode::Object(
            map.into_iter()
                .map(|(k, v)| (mapping_key_to_string(k), from_value(v)))
                .collect(),
        ),
        Value::Sequence(seq) => DataNode::Array(seq.into_iter().map(from_value).collect()),
        Value::String(s) => DataNode::Scalar(s),
        Value::Number(n) => DataNode::Scalar(n.to_string()),
        Value::Bool(b) => DataNode::Scalar(b.to_string()),
        Value::Null => DataNode::Null,
        Value::Tagged(tagged) => from_value(tagged.value),
    }
}

fn mapping_key_to_string(key: Value) -> String {
    match key {
        Value::String(s) => s,
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_flat_mapping() {
        let data = parse_yaml("a: 1\nb: x\n").unwrap();
        assert_eq!(
            data,
            DataNode::Object(vec![
                ("a".into(), DataNode::Scalar("1".into())),
                ("b".into(), DataNode::Scalar("x".into())),
            ])
        );
    }

    #[test]
    fn parses_nested_sequence() {
        let data = parse_yaml("- 1\n- - 2\n  - 3\n").unwrap();
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
        assert!(parse_yaml(": : :").is_err());
    }
}
