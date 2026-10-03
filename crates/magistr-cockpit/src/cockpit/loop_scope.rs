//! The user's choice of roadmap work for a loop.

/// A choice made before the loop starts.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LoopScope {
    AllTracks,
    Track(String),
}

impl LoopScope {
    #[must_use]
    pub fn track(&self) -> Option<&str> {
        match self {
            Self::AllTracks => None,
            Self::Track(track) => Some(track),
        }
    }
}
