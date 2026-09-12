//! Plan guard — the plan lives in the execution trace (`ship_set_objective`
//! + `ship_plan`), not in a plan file; this guard keys on that state.

use crate::workflow::guards::{GuardResult, WorkflowGuard};

/// Steers the agent when it is about to execute without a recorded ship
/// plan, or to commit without a recorded quality gate.
///
/// Reads `GuardContext::ship` (a [`magistr_engine::ShipGuardState`] read
/// from `ship_status`), so the guard is IO-free: whoever builds the context
/// asks the ship engine.
pub struct PlanGuard;

impl WorkflowGuard for PlanGuard {
    fn name(&self) -> &str {
        "plan"
    }

    fn pre_session(&self, ctx: &crate::workflow::guards::GuardContext) -> GuardResult {
        match ctx.ship.phase_block("execute") {
            None => GuardResult::ok(self.name()),
            Some(reason) => GuardResult::steer(self.name(), reason),
        }
    }

    fn pre_commit(&self, ctx: &crate::workflow::guards::GuardContext) -> GuardResult {
        match ctx.ship.phase_block("done") {
            None => GuardResult::ok(self.name()),
            Some(reason) => GuardResult::steer(self.name(), reason),
        }
    }
}
