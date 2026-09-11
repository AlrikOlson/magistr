use serde::{Deserialize, Serialize};

/// Evidence strength of a name-resolution-free reference edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchConfidence {
    High,
    Medium,
    Low,
}

impl SearchConfidence {
    pub(super) const fn weight(self) -> f64 {
        match self {
            Self::High => 1.0,
            Self::Medium => 0.6,
            Self::Low => 0.3,
        }
    }
}
