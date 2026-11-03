use magistr_workflow::run::domain::goal::{Goal, GoalStatus};
use magistr_workflow::run::domain::step::Step;

/// The slice of run (execution-trace) state the lifecycle guards key on.
///
/// Read off the run engine's own records — the goal and the steps
/// with the checks on them — so the engine layer stays IO-free: whoever
/// holds the engine (mcp, a test) reads them and hands the guard
/// this value.
///
/// A goal must be set and at least one step planned before the
/// first source edit (`execute`), and at least one check must be
/// recorded before the iteration is declared `done`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RunGuardState {
    /// A goal is defined or active (not completed/abandoned, not absent).
    pub goal_set: bool,
    /// Steps in the current plan (`arrange_steps` adds).
    pub steps_planned: usize,
    /// Checks recorded on the current plan's steps (`check_step`).
    pub checks_recorded: usize,
}

impl RunGuardState {
    /// Read the guard-relevant slice off the run records: whether an
    /// goal is defined or active, how many steps the plan holds, and
    /// how many checks are recorded on them.
    ///
    /// Checks are counted only on the current plan's steps: after
    /// `run_finish` a fresh goal clears its steps, and a stale
    /// check from the finished cycle must not satisfy the next iteration's
    /// `done` guard.
    #[must_use]
    pub fn from_run(goal: Option<&Goal>, steps: &[Step]) -> Self {
        Self {
            goal_set: goal
                .is_some_and(|o| matches!(o.status, GoalStatus::Open | GoalStatus::Active)),
            steps_planned: steps.len(),
            checks_recorded: steps.iter().map(|step| step.checks.len()).sum(),
        }
    }

    /// A goal is set and at least one step is planned.
    #[must_use]
    pub fn has_plan(&self) -> bool {
        self.goal_set && self.steps_planned > 0
    }

    /// At least one check has been recorded on the current plan.
    #[must_use]
    pub fn has_check(&self) -> bool {
        self.checks_recorded > 0
    }

    /// Why a transition into `to_phase` is refused, or `None` when it may
    /// proceed. `execute` and `verify` need a plan; `done` needs a recorded
    /// check. `research` and `plan` are always open — that is where the plan
    /// gets written.
    #[must_use]
    pub fn phase_block(&self, to_phase: &str) -> Option<String> {
        match to_phase {
            "execute" | "verify" if !self.has_plan() => Some(format!(
                "No run plan recorded (goal_set={}, steps_planned={}). Call \
                 run_open and arrange_steps (at least one step) before entering \
                 the {to_phase} phase — the steps live in the run, not in a \
                 plan file.",
                self.goal_set, self.steps_planned
            )),
            "done" if !self.has_check() => Some(
                "No check recorded on this plan. Run the checks through \
                 magistr_check (recorded via check_step) or check_step(command:) before \
                 declaring the iteration done."
                    .to_owned(),
            ),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "run_guard_state.tests.rs"]
mod tests;
