pub mod csv;
pub mod json;
pub mod toml;
pub mod xml;
pub mod yaml;

use crate::model::DataNode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Yaml,
    Xml,
    Csv,
    Toml,
}

impl Format {
    pub const ALL: [Format; 5] = [
        Format::Json,
        Format::Yaml,
        Format::Xml,
        Format::Csv,
        Format::Toml,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Format::Json => "JSON",
            Format::Yaml => "YAML",
            Format::Xml => "XML",
            Format::Csv => "CSV",
            Format::Toml => "TOML",
        }
    }

    pub fn sample(&self) -> &'static str {
        match self {
            Format::Json => json::SAMPLE,
            Format::Yaml => yaml::SAMPLE,
            Format::Xml => xml::SAMPLE,
            Format::Csv => csv::SAMPLE,
            Format::Toml => toml::SAMPLE,
        }
    }

    pub fn from_label(label: &str) -> Option<Format> {
        Format::ALL.into_iter().find(|f| f.label() == label)
    }
}

pub fn parse(format: Format, input: &str) -> Result<DataNode, String> {
    match format {
        Format::Json => json::parse_json(input),
        Format::Yaml => yaml::parse_yaml(input),
        Format::Xml => xml::parse_xml(input),
        Format::Csv => csv::parse_csv(input),
        Format::Toml => toml::parse_toml(input),
    }
}
