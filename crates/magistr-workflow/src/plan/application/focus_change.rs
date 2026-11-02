use crate::plan::domain::{FeatureFrontier, Focus};

/// A focus mutation with the frontier observed under the same native lock.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(untagged)]
pub enum FocusChange {
    Set {
        team: String,
        focus: Focus,
        frontier: FeatureFrontier,
    },
    Cleared {
        team: String,
        focus: Option<Focus>,
        released: bool,
    },
}
