//! The user's choice of plan work for a loop.

/// A choice made before the loop starts.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LoopScope {
    AllFeatures,
    Feature(String),
}

impl LoopScope {
    #[must_use]
    pub fn feature(&self) -> Option<&str> {
        match self {
            Self::AllFeatures => None,
            Self::Feature(feature) => Some(feature),
        }
    }
}
