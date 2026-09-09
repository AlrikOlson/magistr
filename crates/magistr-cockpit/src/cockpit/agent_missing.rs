//! The agent binary the entry point could not find.

/// The agent is not on PATH — what the entry point found before the
/// surface came up, the way it found [`LauncherData`](super::LauncherData)'s
/// rows. The cockpit never searches PATH itself; it is told, and the
/// bad day ([`BadDay`](super::BadDay)) reads it.
///
/// `last_seen` is pre-formatted by whoever knew the wall clock — a
/// `09:14` — because the cockpit keeps no clock of its own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentMissing {
    binary: String,
    last_seen: Option<String>,
}

impl AgentMissing {
    /// `binary` is not on PATH.
    #[must_use]
    pub fn new(binary: impl Into<String>) -> Self {
        Self {
            binary: binary.into(),
            last_seen: None,
        }
    }

    /// When it was last detected, as the entry point formatted it.
    #[must_use]
    pub fn seen(mut self, stamp: impl Into<String>) -> Self {
        self.last_seen = Some(stamp.into());
        self
    }

    /// The binary's name.
    #[must_use]
    pub fn binary(&self) -> &str {
        &self.binary
    }

    /// When it was last detected, if ever.
    #[must_use]
    pub fn last_seen(&self) -> Option<&str> {
        self.last_seen.as_deref()
    }
}
