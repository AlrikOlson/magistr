use super::{LoopCmd, WorkPhase};
use magistr_domain::{identity::RunId, workflow::RunMode};

/// Complete current work facts. Readers retain these outside bounded messages.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WorkRun {
    pub id: RunId,
    pub mode: RunMode,
    pub track: Option<String>,
    pub chunk: Option<String>,
    pub iteration: u32,
    pub limit: u32,
    pub phase: WorkPhase,
    pub requested: Option<LoopCmd>,
}
impl WorkRun {
    #[must_use]
    pub const fn is_active(&self) -> bool {
        !matches!(self.phase, WorkPhase::Ended { .. })
    }
}
