//! Observations produced by resident session, workflow and source owners.
use super::{CheckpointEvent, Cmd, LoopEvent, WorkbenchEvent};
use crate::{
    engine::EngineEvent,
    job::{JobId, JobOutput},
};
use magistr_application::sessions::AgentEvent;

// Native store events contain complete records and are consumed together.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    InputAccepted(magistr_application::sessions::PromptReceipt),
    PromptReplace(String),
    Agent(AgentEvent),
    CommandFailed { cmd: Cmd, error: String },
    RoadmapRequestQueued,
    RoadmapRequestStarted,
    RoadmapRequestCancelled,
    Engine(EngineEvent),
    Job { id: JobId, output: JobOutput },
    Loop(LoopEvent),
    Workbench(WorkbenchEvent),
    Checkpoint(CheckpointEvent),
}
