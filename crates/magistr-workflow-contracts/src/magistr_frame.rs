//! What the host serving the feed says about itself.

use serde::{Deserialize, Serialize};

use super::gate_output::GateOutput;
use magistr_domain::workflow::WorkflowPhase;

/// The `magistr` family's frames: facts about the run that no store
/// family records, said by the process that serves the socket
/// (`magistr mcp-serve`) or posted in-process by the loop driver.
///
/// Wire form is the same internally tagged JSON object every family
/// uses — `{"type": "gate_output", …}` — so a subscriber decodes it
/// exactly where it decodes the store families.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MagistrFrame {
    /// One chunk a running gate wrote to its pty, keyed by the gate.
    GateOutput(GateOutput),
    /// The agent announced a workflow phase (`magistr_set_phase`). The
    /// phase travels typed; the action the service also records on the
    /// ship trace is narrative, and nothing reads the phase back out of
    /// its words.
    PhaseSet {
        /// The phase entered.
        phase: WorkflowPhase,
    },
}
