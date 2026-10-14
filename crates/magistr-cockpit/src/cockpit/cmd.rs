//! Addressable actions handled by resident session and workflow services.
use super::{CheckpointCmd, EngineCmd, KillCmd, LaunchAction, LoopCmd, WorkbenchCmd};
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Cmd {
    Quit,
    Prompt(magistr_application::sessions::PromptInput),
    RoadmapPrompt(magistr_application::sessions::PromptInput),
    CancelRoadmapPrompt,
    Cancel,
    Permission { id: u64, option_id: String },
    Constraint { id: u64, condition: String },
    Delegate(String),
    Loop(LoopCmd),
    Launch(LaunchAction),
    Kill(KillCmd),
    Workbench(WorkbenchCmd),
    Engine(EngineCmd),
    Checkpoint(CheckpointCmd),
    Reconnect,
    Rename(String),
    Configure { id: String, value: String },
}
impl Cmd {
    #[must_use]
    pub fn undelivered(self) -> super::Msg {
        super::Msg::CommandFailed {
            cmd: self,
            error: "The session connection closed. Reopen the session and try again.".into(),
        }
    }
}
