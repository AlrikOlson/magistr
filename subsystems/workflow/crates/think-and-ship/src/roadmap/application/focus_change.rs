use crate::roadmap::domain::Focus;

/// A focus mutation with the frontier observed under the same native lock.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(untagged)]
pub enum FocusChange {
    Set {
        lane: String,
        focus: Focus,
        frontier: serde_json::Value,
        persistent: bool,
    },
    Cleared {
        lane: String,
        focus: Option<Focus>,
        released: bool,
        persistent: bool,
    },
}
