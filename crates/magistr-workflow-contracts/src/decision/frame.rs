//! Typed mutation records; emission belongs to the workflow host.

use super::{Note, ThreadStatus};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DecisionFrame {
    /// A new note was appended to the log (either main line or thread).
    NoteAppended {
        #[serde(skip_serializing_if = "Option::is_none")]
        log_id: Option<String>,
        note: Box<Note>,
    },
    /// An older note's `revised_by` pointer was set. The revising note
    /// itself is delivered via a separate [`Self::NoteAppended`] frame.
    NoteRevised {
        #[serde(skip_serializing_if = "Option::is_none")]
        log_id: Option<String>,
        revised_note: u32,
        by_note: u32,
    },
    /// A thread transitioned to a new status. `merged_into` is set only
    /// when `status == Merged` and the caller named a synthesis note.
    ThreadStatusChanged {
        #[serde(skip_serializing_if = "Option::is_none")]
        log_id: Option<String>,
        thread_id: String,
        status: ThreadStatus,
        #[serde(skip_serializing_if = "Option::is_none")]
        merged_into: Option<u32>,
    },
    /// The `pinned` flag on a note changed.
    PinChanged {
        #[serde(skip_serializing_if = "Option::is_none")]
        log_id: Option<String>,
        note: u32,
        pinned: bool,
    },
    /// The most-recent note's `estimated_total` was revised in place.
    EstimateRevised {
        #[serde(skip_serializing_if = "Option::is_none")]
        log_id: Option<String>,
        old: u32,
        new: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// The entire log was wiped.
    Cleared,
}
