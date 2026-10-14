use super::LoopCmd;
use magistr_domain::identity::RunId;

/// A boundary request captured for one exact run, independent of its current conversation.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RunCommand {
    pub run_id: RunId,
    pub action: LoopCmd,
}
