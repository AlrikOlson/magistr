//! What an owner answers a command with.

use magistr_workflow::roadmap::domain::Chunk;
use magistr_workflow::ship::domain::action::Action;
use magistr_workflow::ship::domain::check::Check;
use magistr_workflow::ship::domain::task::Task;
use magistr_workflow::signal::domain::Signal;
use serde::{Deserialize, Serialize};

/// The value an [`OwnerCommand`](super::OwnerCommand) produces, exactly
/// what the owner's own [`NativeEngines`](super::NativeEngines) verb
/// returned. A reasoning step's outcome travels as its text, since the
/// library's result type has no wire form.
// Externally tagged: several variants wrap a primitive, which an internal
// tag cannot carry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerReply {
    /// The chunk as it stands after the command. Boxed: a chunk is the
    /// largest reply by far.
    Chunk(Box<Chunk>),
    /// The task as it stands after the command.
    Task(Task),
    /// The action recorded.
    Action(Action),
    /// The check recorded.
    Check(Check),
    /// The signal as it stands after the command.
    Signal(Signal),
    /// The reasoning engine's answer to a recorded step: the formatted
    /// acceptance, or the refusal it printed.
    Step(Result<String, String>),
    /// Whether the step is pinned after the command.
    Pinned(bool),
    /// How many stale tasks a fresh objective cleared.
    Cleared(usize),
    /// A document: a checkpoint report or a ship report.
    Report(serde_json::Value),
    /// The command produced nothing to show.
    Done,
}
