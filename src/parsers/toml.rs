use crate::model::DataNode;
use toml::Value;

pub const SAMPLE: &str = r#"name = "json-ruster"
version = "0.1.0"
tags = ["json", "rust", "wasm"]

[author]
name = "John"
active = true

[author.address]
city = "Paris"
"#;

pub fn parse_toml(input: &str) -> Result<DataNode, String> {
    let value: Value = toml::from_str(input).map_err(|e| e.to_string())?;
    Ok(from_value(value))
}

fn from_value(value: Value) -> DataNode {
    match value {
        Value::Table(map) => {
            DataNode::Object(map.into_iter().map(|(k, v)| (k, from_value(v))).collect())
        }
        Value::Array(arr) => DataNode::Array(arr.into_iter().map(from_value).collect()),
        Value::String(s) => DataNode::Scalar(s),
        Value::Integer(n) => DataNode::Scalar(n.to_string()),
        Value::Float(n) => DataNode::Scalar(n.to_string()),
        Value::Boolean(b) => DataNode::Scalar(b.to_string()),
        Value::Datetime(dt) => DataNode::Scalar(dt.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_flat_table() {
        let data = parse_toml("a = 1\nb = \"x\"\n").unwrap();
        assert_eq!(
            data,
            DataNode::Object(vec![
                ("a".into(), DataNode::Scalar("1".into())),
                ("b".into(), DataNode::Scalar("x".into())),
            ])
        );
    }

    #[test]
    fn parses_nested_table() {
        let data = parse_toml("[author]\nname = \"A\"\n").unwrap();
        assert_eq!(
            data,
            DataNode::Object(vec![(
                "author".into(),
                DataNode::Object(vec![("name".into(), DataNode::Scalar("A".into()))]),
            )])
        );
    }

    #[test]
    fn reports_parse_errors() {
        assert!(parse_toml("not = = toml").is_err());
    }
}
