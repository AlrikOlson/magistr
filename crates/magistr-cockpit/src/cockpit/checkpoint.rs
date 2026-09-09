//! One state on the session's spine, as the rail shows it.

use magistr_engine::CheckpointReason;

use super::checkpoint_state::CheckpointState;
use crate::theme::Icons;

/// A checkpoint as the cockpit knows it: the name it was given, why it
/// was taken, what it was taken before, and where the port got to.
///
/// Deliberately *not* the engine's `SessionSnapshot`. The cockpit shows
/// a chain and asks for verbs on it; the state itself lives behind the
/// checkpoint port, in the process that can actually resume a workflow
/// and open a worktree. A copy of the snapshot here would be a second
/// place for the session's own state to live, which is the thing this
/// crate keeps out of the model everywhere else.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkpoint {
    id: String,
    reason: CheckpointReason,
    label: String,
    state: CheckpointState,
    /// The gates the iteration ran before this boundary, in order, and
    /// whether each passed — the `✓✓✓` the design's row wears. Empty
    /// on a risky or manual checkpoint, which no gates precede.
    gates: Vec<(String, bool)>,
}

impl Checkpoint {
    /// A checkpoint asked for and not yet answered.
    #[must_use]
    pub fn asked(
        id: impl Into<String>,
        reason: CheckpointReason,
        label: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            reason,
            label: label.into(),
            state: CheckpointState::Saving,
            gates: Vec::new(),
        }
    }

    /// This checkpoint wearing the verdicts of the gates that ran
    /// before it.
    #[must_use]
    pub fn with_gates(mut self, gates: Vec<(String, bool)>) -> Self {
        self.gates = gates;
        self
    }

    /// The gates that preceded it, in order, and whether each passed.
    #[must_use]
    pub fn gates(&self) -> &[(String, bool)] {
        &self.gates
    }

    /// The verdicts as one run of glyphs — `✓✓✓`, `✓✓✗` — in `icons`'
    /// vocabulary, or `None` when no gates preceded this state, so a
    /// row that has none to show prints nothing rather than an empty
    /// separator.
    #[must_use]
    pub fn gate_marks(&self, icons: &Icons) -> Option<String> {
        if self.gates.is_empty() {
            return None;
        }
        Some(
            self.gates
                .iter()
                .map(|(_, passed)| icons.gate(Some(*passed)))
                .collect(),
        )
    }

    /// The name the rail and the card both print.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Why it was taken.
    #[must_use]
    pub const fn reason(&self) -> CheckpointReason {
        self.reason
    }

    /// What it was taken before, in the reader's words.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Where the port got to.
    #[must_use]
    pub const fn state(&self) -> &CheckpointState {
        &self.state
    }

    /// The port took it.
    pub fn save(&mut self, head: Option<String>, verified: bool) {
        self.state = CheckpointState::Saved { head, verified };
    }

    /// The port refused.
    pub fn fail(&mut self, error: impl Into<String>) {
        self.state = CheckpointState::Failed(error.into());
    }

    /// Whether this is a state a fork could reproduce.
    #[must_use]
    pub const fn is_verified(&self) -> bool {
        matches!(self.state, CheckpointState::Saved { verified: true, .. })
    }

    /// The rail's one-line checkpoint label, such as `cp-3 · rm -rf build`.
    ///
    /// A saved checkpoint shows its label, falling back to its reason when no
    /// label was supplied. A pending checkpoint shows its save status. Keeping
    /// one detail after the name avoids wrapping the narrow rail.
    #[must_use]
    pub fn row(&self) -> String {
        if !self.is_verified() {
            return format!("{} · {}", self.id, self.state.label());
        }
        let what = if self.label.is_empty() {
            self.reason.to_string()
        } else {
            self.label.clone()
        };
        format!("{} · {what}", self.id)
    }
}

#[cfg(test)]
#[path = "checkpoint.tests.rs"]
mod tests;
