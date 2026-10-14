use crate::roadmap::domain::{Focus, WorkstreamFrontier};

/// A focus mutation with the frontier observed under the same native lock.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(untagged)]
pub enum FocusChange {
    Set {
        lane: String,
        focus: Focus,
        frontier: WorkstreamFrontier,
    },
    Cleared {
        lane: String,
        focus: Option<Focus>,
        released: bool,
    },
}
