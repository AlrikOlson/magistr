//! Typed shapes of one guest conversation: the `done` frame and how a round
//! ended.
use serde_json::Value;

use crate::contracts::WorkspaceError;

/// The guest's final frame: its return value or throw plus the queued ops in
/// guest call order (edits first, then file ops — the shim keeps two arrays).
pub(super) struct GuestDone {
    pub value: Value,
    pub threw: Option<String>,
    pub edits: Vec<Value>,
    pub files: Vec<Value>,
}

/// How the re-entry loop ended.
pub(super) enum GuestRun {
    Done {
        done: GuestDone,
        stdout: String,
        stderr: String,
    },
    /// The user's program failed: data, like a failing test.
    Threw {
        error: String,
        stdout: String,
        stderr: String,
    },
    /// A typed refusal (sandbox kill, capability violation, dead lane).
    Error(WorkspaceError),
}

pub(super) struct GuestLoop {
    pub run: GuestRun,
    pub rounds: u32,
    pub host_calls: u32,
    pub fuel_used: u64,
}
