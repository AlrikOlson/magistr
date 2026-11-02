//! The decision-log verbs an attached client can ask its owner for.

use magistr_workflow::decision::domain::Note;
use serde::{Deserialize, Serialize};

/// One decision-log mutation, argument for argument the
/// [`NativeEngines`](super::NativeEngines) verb of the same name.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "verb", rename_all = "snake_case")]
pub enum DecisionCommand {
    /// `decision_record`. Boxed: a note is by far the largest payload
    /// on the wire.
    Record { note: Box<Note> },
    /// `decision_pin`.
    Pin { note: u32, pinned: bool },
    /// `decision_review`.
    Review,
}
