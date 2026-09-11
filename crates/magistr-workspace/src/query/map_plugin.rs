use serde::Serialize;

/// Installed-plugin inventory row; trust is a composition-time fact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MapPlugin {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub source: String,
    pub trust: String,
}
