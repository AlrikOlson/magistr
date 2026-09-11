use serde::Serialize;

/// Member dependency with optional dev/build scope and observed import count.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MapDependency {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<usize>,
}
