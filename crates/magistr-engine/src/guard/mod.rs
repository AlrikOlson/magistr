//! Workflow guard abstractions — lifecycle enforcement for plan-execute-verify.
//!
//! Guards fire at session and commit boundaries to steer the agent or warn
//! the orchestrator. Concrete guard implementations live in `magistr-cli`.

pub mod guard_action;
pub mod guard_context;
pub mod guard_result;
pub mod guards_config;
pub mod ship_guard_state;
pub mod workflow_guard;

pub use guard_action::GuardAction;
pub use guard_context::GuardContext;
pub use guard_result::GuardResult;
pub use guards_config::GuardsConfig;
pub use ship_guard_state::ShipGuardState;
pub use workflow_guard::WorkflowGuard;

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
