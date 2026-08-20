use crate::model::DataNode;
use roxmltree::{Document, Node};
use std::collections::HashMap;

pub const SAMPLE: &str = r#"<project name="json-ruster" version="0.1.0">
  <tags>
    <tag>json</tag>
    <tag>rust</tag>
    <tag>wasm</tag>
  </tags>
  <author active="true">
    <name>John</name>
    <address>
      <city>Paris</city>
    </address>
  </author>
</project>
"#;

pub fn parse_xml(input: &str) -> Result<DataNode, String> {
    let doc = Document::parse(input).map_err(|e| e.to_string())?;
    Ok(from_element(doc.root_element()))
}

fn from_element(node: Node) -> DataNode {
    let mut fields: Vec<(String, DataNode)> = node
        .attributes()
        .map(|attr| {
            (
                format!("@{}", attr.name()),
                DataNode::Scalar(attr.value().to_string()),
            )
        })
        .collect();

    let child_elements: Vec<Node> = node.children().filter(|c| c.is_element()).collect();

    if child_elements.is_empty() {
        let text = node.text().unwrap_or("").trim().to_string();
        if fields.is_empty() {
            return if text.is_empty() {
                DataNode::Null
            } else {
                DataNode::Scalar(text)
            };
        }
        if !text.is_empty() {
            fields.push(("#text".to_string(), DataNode::Scalar(text)));
        }
        return DataNode::Object(fields);
    }

    // Repeated child tags become an array under that tag name.
    let mut order: Vec<String> = Vec::new();
    let mut grouped: HashMap<String, Vec<DataNode>> = HashMap::new();
    for child in child_elements {
        let name = child.tag_name().name().to_string();
        if !grouped.contains_key(&name) {
            order.push(name.clone());
        }
        grouped.entry(name).or_default().push(from_element(child));
    }

    for name in order {
        let mut values = grouped.remove(&name).unwrap();
        let value = if values.len() == 1 {
            values.pop().unwrap()
        } else {
            DataNode::Array(values)
        };
        fields.push((name, value));
    }

    DataNode::Object(fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_attributes_and_text() {
        let data = parse_xml(r#"<person id="1">Bob</person>"#).unwrap();
        assert_eq!(
            data,
            DataNode::Object(vec![
                ("@id".into(), DataNode::Scalar("1".into())),
                ("#text".into(), DataNode::Scalar("Bob".into())),
            ])
        );
    }

    #[test]
    fn repeated_tags_become_an_array() {
        let data = parse_xml("<root><item>a</item><item>b</item></root>").unwrap();
        assert_eq!(
            data,
            DataNode::Object(vec![(
                "item".into(),
                DataNode::Array(vec![
                    DataNode::Scalar("a".into()),
                    DataNode::Scalar("b".into()),
                ]),
            )])
        );
    }

    #[test]
    fn reports_parse_errors() {
        assert!(parse_xml("<unclosed>").is_err());
    }
}
