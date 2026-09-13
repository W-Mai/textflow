//! Output structures for features.json / docs.json.

use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct CrateInfo {
    pub package: String,
    pub lib: String,
    pub version: String,
}

#[derive(Clone, Serialize)]
pub struct Feature {
    pub name: String,
    /// Direct feature edges only; consumers compute the transitive closure.
    pub requires: Vec<String>,
    pub description: String,
}

#[derive(Clone, Serialize)]
pub struct FeatureFile {
    #[serde(rename = "crate")]
    pub crate_info: CrateInfo,
    pub features: Vec<Feature>,
}

#[derive(Clone, Serialize)]
pub struct DocsFile {
    pub modules: Vec<Module>,
}

#[derive(Clone, Serialize)]
pub struct Module {
    /// Empty string for the crate root.
    pub name: String,
    pub doc: String,
    pub features: Vec<String>,
    pub source: String,
    pub items: Vec<Item>,
}

#[derive(Clone, Serialize)]
pub struct Item {
    pub kind: String,
    pub name: String,
    pub signature: String,
    pub doc: String,
    pub features: Vec<String>,
    pub line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<Variant>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<Field>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub methods: Vec<Method>,
}

#[derive(Clone, Serialize)]
pub struct Variant {
    pub name: String,
    pub doc: String,
    pub line: usize,
}

#[derive(Clone, Serialize)]
pub struct Field {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub doc: String,
    pub line: usize,
}

#[derive(Clone, Serialize)]
pub struct Method {
    pub name: String,
    pub signature: String,
    pub doc: String,
    pub features: Vec<String>,
    pub line: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub doctests: Vec<String>,
}
