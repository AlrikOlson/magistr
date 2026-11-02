//! What an owner answers a command with.

use magistr_workflow::roadmap::domain::Chunk;
use magistr_workflow::run::domain::check::Check;
use magistr_workflow::run::domain::progress::Progress;
use magistr_workflow::run::domain::step::Step;
use magistr_workflow::signal::application::SignalPromotion;
use magistr_workflow::signal::domain::Signal;
use magistr_workflow_contracts::Observed;
use magistr_workflow_contracts::think::RecordedStep;
use serde::{Deserialize, Serialize};

/// The value an [`OwnerCommand`](super::OwnerCommand) produces, exactly
/// what the owner's own [`NativeEngines`](super::NativeEngines) verb
/// returned, including the provenance of each committed document.
// Externally tagged: several variants wrap a primitive, which an internal
// tag cannot carry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerReply {
    /// The chunk as it stands after the command. Boxed: a chunk is the
    /// one of the larger reply payloads.
    Chunk(Box<Observed<Chunk>>),
    /// The step as it stands after the command.
    Step(Observed<Step>),
    /// The progress recorded.
    Progress(Observed<Progress>),
    /// The check recorded.
    Check(Observed<Check>),
    /// The signal as it stands after the command.
    Signal(Observed<Signal>),
    /// The reasoning engine's answer to a recorded step: the formatted
    /// acceptance, or the refusal it printed.
    ThinkStep(Box<RecordedStep>),
    /// Whether the step is pinned after the command.
    Pinned(Observed<bool>),
    /// How many stale steps a fresh goal cleared.
    Cleared(Observed<usize>),
    /// A document: a checkpoint report or a run report.
    Report(serde_json::Value),
    /// The command produced nothing to show.
    Done(Observed<()>),
    /// Both owning records acknowledge their own committed files.
    Promotion(Box<SignalPromotion>),
}
