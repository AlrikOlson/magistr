use crate::roadmap::domain::Focus;

/// A lane's read-only focus and the existing extensible frontier projection.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FocusRead {
    pub lane: String,
    pub focus: Option<Focus>,
    pub frontier: serde_json::Value,
    pub groups: Vec<String>,
    pub persistent: bool,
}
