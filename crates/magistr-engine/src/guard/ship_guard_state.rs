use magistr_workflow::ship::domain::objective::{Objective, ObjectiveStatus};
use magistr_workflow::ship::domain::task::Task;

/// The slice of ship (execution-trace) state the lifecycle guards key on.
///
/// Read off the ship engine's own records — the objective and the tasks
/// with the checks on them — so the engine layer stays IO-free: whoever
/// holds the engine (mcp, a test) reads them and hands the guard
/// this value.
///
/// An objective must be set and at least one task planned before the
/// first source edit (`execute`), and at least one quality gate must be
/// recorded before the iteration is declared `done`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShipGuardState {
    /// An objective is defined or active (not completed/abandoned, not absent).
    pub objective_set: bool,
    /// Tasks in the current plan (`ship_plan` adds).
    pub tasks_planned: usize,
    /// Quality gates recorded on the current plan's tasks (`ship_check`).
    pub checks_recorded: usize,
}

impl ShipGuardState {
    /// Read the guard-relevant slice off the ship records: whether an
    /// objective is defined or active, how many tasks the plan holds, and
    /// how many checks are recorded on them.
    ///
    /// Checks are counted only on the current plan's tasks: after
    /// `ship_finalize` a fresh objective clears its tasks, and a stale
    /// check from the finished cycle must not satisfy the next iteration's
    /// `done` guard.
    #[must_use]
    pub fn from_ship(objective: Option<&Objective>, tasks: &[Task]) -> Self {
        Self {
            objective_set: objective.is_some_and(|o| {
                matches!(o.status, ObjectiveStatus::Defined | ObjectiveStatus::Active)
            }),
            tasks_planned: tasks.len(),
            checks_recorded: tasks.iter().map(|task| task.checks.len()).sum(),
        }
    }

    /// An objective is set and at least one task is planned.
    #[must_use]
    pub fn has_plan(&self) -> bool {
        self.objective_set && self.tasks_planned > 0
    }

    /// At least one quality gate has been recorded on the current plan.
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
                "No ship plan recorded (objective_set={}, tasks_planned={}). Call \
                 ship_set_objective and ship_plan (at least one task) before entering \
                 the {to_phase} phase — the plan lives in the execution trace, not in a \
                 plan file.",
                self.objective_set, self.tasks_planned
            )),
            "done" if !self.has_check() => Some(
                "No quality gate recorded on this plan. Run the gates through \
                 magistr_gate (recorded via ship_check) or ship_check(command:) before \
                 declaring the iteration done."
                    .to_owned(),
            ),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "ship_guard_state.tests.rs"]
mod tests;
