//! Plan guard — the plan lives in the run (`run_open`
//! + `arrange_steps`), not in a plan file; this guard keys on that state.

use crate::workflow::guards::{GuardResult, WorkflowGuard};

/// Steers the agent when it is about to execute without a recorded run
/// plan, or to commit without a recorded check.
///
/// Reads `GuardContext::run` (a [`magistr_engine::RunGuardState`] read
/// from `run_status`), so the guard is IO-free: whoever builds the context
/// asks the run engine.
pub struct PlanGuard;

impl WorkflowGuard for PlanGuard {
    fn name(&self) -> &str {
        "plan"
    }

    fn pre_session(&self, ctx: &crate::workflow::guards::GuardContext) -> GuardResult {
        match ctx.run.phase_block("execute") {
            None => GuardResult::ok(self.name()),
            Some(reason) => GuardResult::steer(self.name(), reason),
        }
    }

    fn pre_commit(&self, ctx: &crate::workflow::guards::GuardContext) -> GuardResult {
        match ctx.run.phase_block("done") {
            None => GuardResult::ok(self.name()),
            Some(reason) => GuardResult::steer(self.name(), reason),
        }
    }
}
