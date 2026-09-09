use magistr_domain::gate::GateOutcome;

use crate::CommandReceipt;

use super::{CheckRecording, CheckTarget};

/// Settled execution and independently reported native persistence evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckExecution {
    pub receipt: CommandReceipt,
    pub gate: CheckTarget,
    pub outcome: GateOutcome,
    pub recording: CheckRecording,
}
