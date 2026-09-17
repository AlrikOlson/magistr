//! Workflow event family names on the shared wire.
use serde::{Deserialize, Serialize};

/// Identifies which family emitted a frame. Wire form is the lowercase
/// name (`"think"` / `"ship"` / `"roadmap"` / `"signal"` / `"magistr"`).
///
/// `Magistr` is the host's own family: what the process serving the
/// socket says about itself — a running gate's bytes, the phase the agent
/// announced — beside the four store families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Think,
    Ship,
    Roadmap,
    Signal,
    Magistr,
}

impl Family {
    /// Every family, in declaration order.
    pub const ALL: [Family; 5] = [
        Family::Think,
        Family::Ship,
        Family::Roadmap,
        Family::Signal,
        Family::Magistr,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Think => "think",
            Self::Ship => "ship",
            Self::Roadmap => "roadmap",
            Self::Signal => "signal",
            Self::Magistr => "magistr",
        }
    }

    /// The family whose wire name is `name`, if any.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|family| family.as_str() == name)
    }
}

/// Owner epoch wire key.
pub const EPOCH_KEY: &str = "epoch";
/// Publisher sequence wire key.
pub const SEQ_KEY: &str = "seq";

#[cfg(test)]
#[path = "family.tests.rs"]
mod tests;
