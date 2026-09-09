//! The ship objective and its plan, as the rail shows them.

use serde_json::Value;

use super::plan_task::PlanTask;
use super::task_state::TaskState;

/// The objective and the planned tasks of the running iteration.
///
/// Seeded from a `ship_status` document at start
/// ([`ShipView::from_status`] — state recorded before the feed
/// connected) and folded from the ship family's broadcast frames after
/// ([`ShipView::apply`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShipView {
    objective: Option<String>,
    scope: Option<String>,
    shipped: bool,
    tasks: Vec<PlanTask>,
}

impl ShipView {
    /// Read the plan out of a `ship_status` document.
    #[must_use]
    pub fn from_status(status: &Value) -> Self {
        let objective = &status["objective"];
        let (description, scope) = objective_parts(objective);
        let checks = status["checks"].as_array().cloned().unwrap_or_default();
        Self {
            objective: description,
            scope,
            shipped: objective["status"].as_str() == Some("completed"),
            tasks: status["task_list"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|task| task_row(task, &checks))
                .collect(),
        }
    }

    /// Fold one ship-family broadcast frame in.
    pub fn apply(&mut self, kind: &str, payload: &Value) {
        if let Some(state) = task_transition(kind) {
            if let Some(task) = self.task_mut(payload) {
                task.state = state;
            }
            return;
        }
        match kind {
            "objective_set" => self.begin_objective(&payload["objective"]),
            "task_added" => self.add_task(payload),
            "check_recorded" => self.count_check(payload),
            "objective_shipped" => self.shipped = true,
            "cleared" => *self = Self::default(),
            _ => {}
        }
    }

    /// The objective's description, once one is set.
    #[must_use]
    pub fn objective(&self) -> Option<&str> {
        self.objective.as_deref()
    }

    /// The objective's scope, typically `chunk:<id>`, when it has one.
    #[must_use]
    pub fn scope(&self) -> Option<&str> {
        self.scope.as_deref()
    }

    /// Whether `ship_finalize` closed the objective.
    #[must_use]
    pub const fn is_shipped(&self) -> bool {
        self.shipped
    }

    /// The tasks in plan order.
    #[must_use]
    pub fn tasks(&self) -> &[PlanTask] {
        &self.tasks
    }

    /// A fresh objective replaces the goal. It clears the old plan only
    /// when the previous objective had shipped; an in-flight objective
    /// keeps its tasks (mirrors `ship_set_objective`).
    fn begin_objective(&mut self, objective: &Value) {
        (self.objective, self.scope) = objective_parts(objective);
        if self.shipped {
            self.tasks.clear();
        }
        self.shipped = false;
    }

    fn add_task(&mut self, payload: &Value) {
        let id = payload["task_id"].as_str().unwrap_or_default();
        if self.tasks.iter().any(|task| task.id == id) {
            return;
        }
        self.tasks.push(PlanTask {
            id: id.to_owned(),
            title: payload["title"].as_str().unwrap_or_default().to_owned(),
            ..PlanTask::default()
        });
    }

    fn count_check(&mut self, payload: &Value) {
        let passed = payload["check"]["passed"].as_bool().unwrap_or(false);
        let Some(task) = self.task_mut(payload) else {
            return;
        };
        let count = if passed {
            &mut task.checks_passed
        } else {
            &mut task.checks_failed
        };
        *count += 1;
    }

    fn task_mut(&mut self, payload: &Value) -> Option<&mut PlanTask> {
        let id = payload["task_id"].as_str()?;
        self.tasks.iter_mut().find(|task| task.id == id)
    }
}

/// The task state a lifecycle frame moves its task to, for the frames
/// that are one.
fn task_transition(kind: &str) -> Option<TaskState> {
    match kind {
        "task_started" => Some(TaskState::Active),
        "task_completed" => Some(TaskState::Completed),
        "task_blocked" => Some(TaskState::Blocked),
        _ => None,
    }
}

/// The description and scope out of an `objective` object; an empty
/// scope reads as none.
fn objective_parts(objective: &Value) -> (Option<String>, Option<String>) {
    let text = |key: &str| objective[key].as_str().map(str::to_owned);
    (text("description"), text("scope").filter(|s| !s.is_empty()))
}

/// One `task_list` entry with the checks recorded on it counted by
/// outcome.
fn task_row(task: &Value, checks: &[Value]) -> PlanTask {
    let id = task["id"].as_str().unwrap_or_default().to_owned();
    let outcomes: Vec<bool> = checks
        .iter()
        .filter(|check| check["task_id"].as_str() == Some(id.as_str()))
        .map(|check| check["passed"].as_bool().unwrap_or(false))
        .collect();
    let passed = outcomes.iter().filter(|&&ok| ok).count();
    PlanTask {
        title: task["title"].as_str().unwrap_or_default().to_owned(),
        state: TaskState::parse(task["status"].as_str().unwrap_or_default()),
        checks_passed: passed as u32,
        checks_failed: (outcomes.len() - passed) as u32,
        id,
    }
}

#[cfg(test)]
#[path = "ship_view.tests.rs"]
mod tests;
