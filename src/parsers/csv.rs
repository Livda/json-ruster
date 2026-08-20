use crate::model::DataNode;

pub const SAMPLE: &str = "name,role,active\nJohn,author,true\nJane,assistant,true\n";

pub fn parse_csv(input: &str) -> Result<DataNode, String> {
    let mut reader = csv::Reader::from_reader(input.as_bytes());
    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| e.to_string())?
        .iter()
        .map(|h| h.to_string())
        .collect();

    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|e| e.to_string())?;
        let fields = headers
            .iter()
            .cloned()
            .zip(record.iter().map(|v| DataNode::Scalar(v.to_string())))
            .collect();
        rows.push(DataNode::Object(fields));
    }

    Ok(DataNode::Array(rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rows_into_objects() {
        let data = parse_csv("a,b\n1,x\n2,y\n").unwrap();
        assert_eq!(
            data,
            DataNode::Array(vec![
                DataNode::Object(vec![
                    ("a".into(), DataNode::Scalar("1".into())),
                    ("b".into(), DataNode::Scalar("x".into())),
                ]),
                DataNode::Object(vec![
                    ("a".into(), DataNode::Scalar("2".into())),
                    ("b".into(), DataNode::Scalar("y".into())),
                ]),
            ])
        );
    }

    #[test]
    fn reports_row_errors() {
        assert!(parse_csv("a,b\n1\n").is_err());
    }
}
