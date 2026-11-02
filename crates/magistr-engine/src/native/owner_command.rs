//! Everything an attached client can ask its owner to do.

use serde::{Deserialize, Serialize};

use super::decision_command::DecisionCommand;
use super::plan_command::PlanCommand;
use super::run_command::RunCommand;
use super::signal_command::SignalCommand;

/// One typed mutation, carried out by the owner's own
/// [`NativeEngines`](super::NativeEngines) through
/// [`NativeEngines::apply`](super::NativeEngines::apply) — the same verbs,
/// the same short critical sections, the same persistence, whichever
/// process asked. The four families mirror the four engines; there is no
/// verb here that the writer does not have, and no way to reach an engine
/// except through one of these.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "family", rename_all = "snake_case")]
pub enum OwnerCommand {
    /// A plan mutation.
    Plan(PlanCommand),
    /// A reasoning-trace mutation.
    Decision(DecisionCommand),
    /// An execution-trace mutation.
    Run(RunCommand),
    /// A signal-inbox mutation.
    Signal(SignalCommand),
}

#[cfg(test)]
#[path = "owner_command.tests.rs"]
mod tests;
