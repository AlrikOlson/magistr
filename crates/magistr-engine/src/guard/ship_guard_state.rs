use serde_json::Value;

/// The slice of ship (execution-trace) state the lifecycle guards key on.
///
/// Parsed from the `ship_status` JSON the ship engine answers — the same
/// document `ship_status` returns over MCP — so the engine layer stays
/// IO-free: whoever holds the engine (mcp-serve, the TUI, a test) reads the
/// status and hands the guard this value.
///
/// The rule it encodes replaces the old "`.magistr/current-plan.md` exists"
/// check: an objective must be set and at least one task planned before the
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
    /// Read the guard-relevant slice out of a `ship_status` document.
    ///
    /// Checks are counted only for tasks present in the current `task_list`:
    /// after `ship_finalize`, `ship_status` still lists the finished cycle's
    /// checks until a fresh objective clears its tasks, and a stale check
    /// must not satisfy the next iteration's `done` guard.
    #[must_use]
    pub fn from_status(status: &Value) -> Self {
        let objective_set = matches!(
            status["objective"]["status"].as_str(),
            Some("defined" | "active")
        );
        let task_ids: Vec<&str> = status["task_list"]
            .as_array()
            .map(|tasks| tasks.iter().filter_map(|t| t["id"].as_str()).collect())
            .unwrap_or_default();
        let checks_recorded = status["checks"]
            .as_array()
            .map(|checks| {
                checks
                    .iter()
                    .filter(|c| {
                        c["task_id"]
                            .as_str()
                            .is_none_or(|id| task_ids.contains(&id))
                    })
                    .count()
            })
            .unwrap_or(0);
        Self {
            objective_set,
            tasks_planned: task_ids.len(),
            checks_recorded,
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
