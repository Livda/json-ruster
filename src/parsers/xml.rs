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

/// `roxmltree::Document::parse` recurses once per nesting level with no
/// depth limit of its own, so a sufficiently deep document overflows the
/// stack and aborts the whole process -- unlike serde_json/serde_yaml/toml,
/// which all guard against this and return a normal parse error instead.
/// A cheap tag-counting pre-scan (not full XML parsing, just enough to
/// bound nesting) rejects pathologically deep input before roxmltree ever
/// sees it.
const MAX_XML_DEPTH: usize = 256;

fn check_xml_depth(input: &str) -> Result<(), String> {
    let mut depth: usize = 0;
    let mut rest = input;
    while let Some(lt) = rest.find('<') {
        rest = &rest[lt..];
        if rest.starts_with("<!--") {
            match rest.find("-->") {
                Some(end) => rest = &rest[end + 3..],
                None => break,
            }
        } else if rest.starts_with("<![CDATA[") {
            match rest.find("]]>") {
                Some(end) => rest = &rest[end + 3..],
                None => break,
            }
        } else if rest.starts_with("<?") {
            match rest.find("?>") {
                Some(end) => rest = &rest[end + 2..],
                None => break,
            }
        } else if rest.starts_with("<!") {
            match rest.find('>') {
                Some(end) => rest = &rest[end + 1..],
                None => break,
            }
        } else if let Some(rest_after_slash) = rest.strip_prefix("</") {
            depth = depth.saturating_sub(1);
            match rest_after_slash.find('>') {
                Some(end) => rest = &rest_after_slash[end + 1..],
                None => break,
            }
        } else {
            match rest.find('>') {
                Some(end) => {
                    if !rest[..end].ends_with('/') {
                        depth += 1;
                        if depth > MAX_XML_DEPTH {
                            return Err(format!(
                                "XML nesting exceeds the maximum supported depth ({MAX_XML_DEPTH})"
                            ));
                        }
                    }
                    rest = &rest[end + 1..];
                }
                None => break,
            }
        }
    }
    Ok(())
}

pub fn parse_xml(input: &str) -> Result<DataNode, String> {
    check_xml_depth(input)?;
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

    #[test]
    fn rejects_pathologically_deep_nesting_instead_of_crashing() {
        let depth = MAX_XML_DEPTH + 10;
        let xml = format!("{}1{}", "<a>".repeat(depth), "</a>".repeat(depth));
        assert!(parse_xml(&xml).is_err());
    }

    #[test]
    fn moderate_nesting_within_the_limit_still_parses() {
        let depth = 50;
        let xml = format!("{}1{}", "<a>".repeat(depth), "</a>".repeat(depth));
        assert!(parse_xml(&xml).is_ok());
    }

    #[test]
    fn comments_and_self_closing_tags_do_not_count_towards_depth() {
        let xml = format!(
            "<!-- {} --><root><br/><br/>1</root>",
            "<a>".repeat(MAX_XML_DEPTH + 10)
        );
        assert!(parse_xml(&xml).is_ok());
    }
}
