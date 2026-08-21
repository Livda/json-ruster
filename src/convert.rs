use crate::model::DataNode;
use crate::parsers::Format;

pub fn convert(data: &DataNode, target: Format) -> Result<String, String> {
    match target {
        Format::Json => to_json(data),
        Format::Yaml => to_yaml(data),
        Format::Xml => to_xml(data),
        Format::Csv => to_csv(data),
        Format::Toml => to_toml(data),
    }
}

/// `DataNode::Scalar` stores every leaf as text (the tree view doesn't need
/// its original type), so round-tripping to a typed format re-infers
/// numbers/booleans/null from the text instead of quoting everything as a
/// string.
pub(crate) enum Scalar {
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
    Text(String),
}

pub(crate) fn infer_scalar(s: &str) -> Scalar {
    if s == "null" {
        Scalar::Null
    } else if let Ok(b) = s.parse::<bool>() {
        Scalar::Bool(b)
    } else if let Ok(i) = s.parse::<i64>() {
        Scalar::Int(i)
    } else if let Ok(f) = s.parse::<f64>() {
        Scalar::Float(f)
    } else {
        Scalar::Text(s.to_string())
    }
}

fn to_json(data: &DataNode) -> Result<String, String> {
    serde_json::to_string_pretty(&to_json_value(data)).map_err(|e| e.to_string())
}

fn to_json_value(node: &DataNode) -> serde_json::Value {
    use serde_json::Value;
    match node {
        DataNode::Object(entries) => Value::Object(
            entries
                .iter()
                .map(|(k, v)| (k.clone(), to_json_value(v)))
                .collect(),
        ),
        DataNode::Array(items) => Value::Array(items.iter().map(to_json_value).collect()),
        DataNode::Null => Value::Null,
        DataNode::Scalar(s) => match infer_scalar(s) {
            Scalar::Int(i) => Value::Number(i.into()),
            Scalar::Float(f) => serde_json::Number::from_f64(f)
                .map(Value::Number)
                .unwrap_or(Value::String(s.clone())),
            Scalar::Bool(b) => Value::Bool(b),
            Scalar::Null => Value::Null,
            Scalar::Text(t) => Value::String(t),
        },
    }
}

fn to_yaml(data: &DataNode) -> Result<String, String> {
    serde_yaml::to_string(&to_yaml_value(data)).map_err(|e| e.to_string())
}

fn to_yaml_value(node: &DataNode) -> serde_yaml::Value {
    use serde_yaml::Value;
    match node {
        DataNode::Object(entries) => Value::Mapping(
            entries
                .iter()
                .map(|(k, v)| (Value::String(k.clone()), to_yaml_value(v)))
                .collect(),
        ),
        DataNode::Array(items) => Value::Sequence(items.iter().map(to_yaml_value).collect()),
        DataNode::Null => Value::Null,
        DataNode::Scalar(s) => match infer_scalar(s) {
            Scalar::Int(i) => Value::Number(i.into()),
            Scalar::Float(f) => Value::Number(f.into()),
            Scalar::Bool(b) => Value::Bool(b),
            Scalar::Null => Value::Null,
            Scalar::Text(t) => Value::String(t),
        },
    }
}

fn to_toml(data: &DataNode) -> Result<String, String> {
    match to_toml_value(data)? {
        toml::Value::Table(table) => {
            toml::to_string_pretty(&toml::Value::Table(table)).map_err(|e| e.to_string())
        }
        _ => Err("TOML export requires a top-level object".to_string()),
    }
}

fn to_toml_value(node: &DataNode) -> Result<toml::Value, String> {
    Ok(match node {
        DataNode::Object(entries) => {
            let mut table = toml::map::Map::new();
            for (k, v) in entries {
                table.insert(k.clone(), to_toml_value(v)?);
            }
            toml::Value::Table(table)
        }
        DataNode::Array(items) => {
            toml::Value::Array(items.iter().map(to_toml_value).collect::<Result<_, _>>()?)
        }
        // TOML has no null: an absent/empty string is the closest fit.
        DataNode::Null => toml::Value::String(String::new()),
        DataNode::Scalar(s) => match infer_scalar(s) {
            Scalar::Int(i) => toml::Value::Integer(i),
            Scalar::Float(f) => toml::Value::Float(f),
            Scalar::Bool(b) => toml::Value::Boolean(b),
            Scalar::Null => toml::Value::String(String::new()),
            Scalar::Text(t) => toml::Value::String(t),
        },
    })
}

fn to_csv(data: &DataNode) -> Result<String, String> {
    let rows = match data {
        DataNode::Array(items) => items,
        _ => return Err("CSV export requires a top-level array of objects".to_string()),
    };

    let mut headers: Vec<String> = Vec::new();
    for row in rows {
        let DataNode::Object(fields) = row else {
            return Err("CSV export requires each array item to be an object".to_string());
        };
        for (k, _) in fields {
            if !headers.contains(k) {
                headers.push(k.clone());
            }
        }
    }

    let mut writer = csv::Writer::from_writer(vec![]);
    writer.write_record(&headers).map_err(|e| e.to_string())?;
    for row in rows {
        if let DataNode::Object(fields) = row {
            let record: Vec<String> = headers
                .iter()
                .map(|h| match fields.iter().find(|(k, _)| k == h) {
                    Some((_, DataNode::Scalar(s))) => s.clone(),
                    _ => String::new(),
                })
                .collect();
            writer.write_record(&record).map_err(|e| e.to_string())?;
        }
    }

    let bytes = writer.into_inner().map_err(|e| e.to_string())?;
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

fn to_xml(data: &DataNode) -> Result<String, String> {
    let mut out = String::new();
    match data {
        DataNode::Array(items) => {
            out.push_str("<root>\n");
            for item in items {
                write_xml_element(&mut out, "item", item, 1);
            }
            out.push_str("</root>\n");
        }
        _ => write_xml_element(&mut out, "root", data, 0),
    }
    Ok(out)
}

fn write_xml_element(out: &mut String, tag: &str, node: &DataNode, indent: usize) {
    let pad = "  ".repeat(indent);
    match node {
        DataNode::Object(fields) => {
            let mut attrs = String::new();
            let mut text: Option<&str> = None;
            let mut children = Vec::new();
            for (k, v) in fields {
                if let Some(name) = k.strip_prefix('@') {
                    if let DataNode::Scalar(s) = v {
                        attrs.push_str(&format!(
                            " {}=\"{}\"",
                            sanitize_xml_name(name),
                            escape_xml(s)
                        ));
                    }
                } else if k == "#text" {
                    if let DataNode::Scalar(s) = v {
                        text = Some(s);
                    }
                } else {
                    children.push((k, v));
                }
            }
            if children.is_empty() {
                match text {
                    Some(t) => {
                        out.push_str(&format!("{pad}<{tag}{attrs}>{}</{tag}>\n", escape_xml(t)))
                    }
                    None => out.push_str(&format!("{pad}<{tag}{attrs} />\n")),
                }
            } else {
                out.push_str(&format!("{pad}<{tag}{attrs}>\n"));
                for (k, v) in children {
                    write_xml_element(out, &sanitize_xml_name(k), v, indent + 1);
                }
                out.push_str(&format!("{pad}</{tag}>\n"));
            }
        }
        DataNode::Array(items) => {
            for item in items {
                write_xml_element(out, tag, item, indent);
            }
        }
        DataNode::Scalar(s) => out.push_str(&format!("{pad}<{tag}>{}</{tag}>\n", escape_xml(s))),
        DataNode::Null => out.push_str(&format!("{pad}<{tag} />\n")),
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// A `DataNode` key becomes an XML tag or attribute name, but arbitrary
/// JSON/YAML/CSV/TOML keys (spaces, quotes, leading digits, ...) aren't
/// valid XML names. Replace invalid characters instead of emitting
/// malformed XML.
fn sanitize_xml_name(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for (i, c) in s.chars().enumerate() {
        let valid = if i == 0 {
            c.is_alphabetic() || c == '_'
        } else {
            c.is_alphanumeric() || matches!(c, '_' | '-' | '.')
        };
        out.push(if valid { c } else { '_' });
    }
    if out.is_empty() {
        out.push('_');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers;

    #[test]
    fn json_round_trips_types() {
        let data =
            parsers::json::parse_json(r#"{"a": 1, "b": true, "c": null, "d": "x"}"#).unwrap();
        let json = to_json(&data).unwrap();
        assert!(json.contains("\"a\": 1"));
        assert!(json.contains("\"b\": true"));
        assert!(json.contains("\"c\": null"));
        assert!(json.contains("\"d\": \"x\""));
    }

    #[test]
    fn json_to_yaml() {
        let data = parsers::json::parse_json(r#"{"a": [1, 2, 3]}"#).unwrap();
        let yaml = convert(&data, Format::Yaml).unwrap();
        let reparsed = parsers::yaml::parse_yaml(&yaml).unwrap();
        assert_eq!(data, reparsed);
    }

    #[test]
    fn csv_requires_array_of_objects() {
        let data = parsers::json::parse_json(r#"{"a": 1}"#).unwrap();
        assert!(to_csv(&data).is_err());
    }

    #[test]
    fn json_array_of_objects_to_csv() {
        let data =
            parsers::json::parse_json(r#"[{"a": "1", "b": "x"}, {"a": "2", "b": "y"}]"#).unwrap();
        let csv = to_csv(&data).unwrap();
        assert_eq!(csv, "a,b\n1,x\n2,y\n");
    }

    #[test]
    fn toml_requires_top_level_object() {
        let data = parsers::json::parse_json(r#"[1, 2]"#).unwrap();
        assert!(to_toml(&data).is_err());
    }

    #[test]
    fn json_to_toml_round_trips() {
        let data = parsers::json::parse_json(r#"{"a": 1, "b": "x"}"#).unwrap();
        let toml_text = convert(&data, Format::Toml).unwrap();
        let reparsed = parsers::toml::parse_toml(&toml_text).unwrap();
        assert_eq!(data, reparsed);
    }

    #[test]
    fn xml_wraps_top_level_array() {
        let data = parsers::json::parse_json(r#"[{"a": "1"}]"#).unwrap();
        let xml = to_xml(&data).unwrap();
        assert!(xml.starts_with("<root>"));
        assert!(xml.contains("<item>"));
    }

    #[test]
    fn xml_round_trips_object() {
        let data = parsers::json::parse_json(r#"{"a": "1", "b": "2"}"#).unwrap();
        let xml = to_xml(&data).unwrap();
        let reparsed = parsers::xml::parse_xml(&xml).unwrap();
        assert_eq!(data, reparsed);
    }

    #[test]
    fn xml_sanitizes_keys_that_are_not_valid_xml_names() {
        let data = parsers::json::parse_json(r#"{"weird key \"x\"": "1", "2nd": "y"}"#).unwrap();
        let xml = to_xml(&data).unwrap();
        // Must still be parseable XML: no raw spaces/quotes left in a tag name.
        assert!(parsers::xml::parse_xml(&xml).is_ok());
        assert!(!xml.contains("<weird key"));
    }
}
