//! The ship objective and its plan, as the rail shows them.

use magistr_engine::ShipFrame;
use magistr_engine::native::{Objective, ObjectiveStatus, Task};

use super::plan_task::PlanTask;
use super::task_state::TaskState;

/// The objective and the planned tasks of the running iteration.
///
/// Seeded from the store's own objective and tasks at start
/// ([`ShipView::from_ship`] — state recorded before the feed
/// connected) and folded from the ship family's events after
/// ([`ShipView::apply`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShipView {
    objective: Option<String>,
    scope: Option<String>,
    shipped: bool,
    tasks: Vec<PlanTask>,
}

impl ShipView {
    /// The plan as the store holds it: the objective, if one is set, and
    /// the tasks in plan order with the checks recorded on each counted
    /// by outcome.
    #[must_use]
    pub fn from_ship(objective: Option<&Objective>, tasks: &[Task]) -> Self {
        let (description, scope) = objective.map(objective_parts).unwrap_or_default();
        Self {
            objective: description,
            scope,
            shipped: objective.is_some_and(|o| o.status == ObjectiveStatus::Completed),
            tasks: tasks.iter().map(task_row).collect(),
        }
    }

    /// Fold one ship-family event in.
    pub fn apply(&mut self, frame: &ShipFrame) {
        match frame {
            ShipFrame::ObjectiveSet { objective } => self.begin_objective(objective),
            ShipFrame::TaskAdded { task_id, title } => self.add_task(task_id, title),
            ShipFrame::TaskStarted { task_id } => self.set_state(task_id, TaskState::Active),
            ShipFrame::TaskCompleted { task_id } => self.set_state(task_id, TaskState::Completed),
            ShipFrame::TaskBlocked { task_id, .. } => self.set_state(task_id, TaskState::Blocked),
            ShipFrame::CheckRecorded { task_id, check } => self.count_check(task_id, check.passed),
            ShipFrame::ObjectiveShipped { .. } => self.shipped = true,
            ShipFrame::Cleared => *self = Self::default(),
            ShipFrame::ActionRecorded { .. } => {}
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

    /// The task by id, when the plan has it — how a task's title is
    /// found for an event that names only the id.
    #[must_use]
    pub fn task(&self, id: &str) -> Option<&PlanTask> {
        self.tasks.iter().find(|task| task.id == id)
    }

    /// The tasks in plan order.
    #[must_use]
    pub fn tasks(&self) -> &[PlanTask] {
        &self.tasks
    }

    /// A fresh objective replaces the goal. It clears the old plan only
    /// when the previous objective had shipped; an in-flight objective
    /// keeps its tasks (mirrors `ship_set_objective`).
    fn begin_objective(&mut self, objective: &Objective) {
        (self.objective, self.scope) = objective_parts(objective);
        if self.shipped {
            self.tasks.clear();
        }
        self.shipped = false;
    }

    fn add_task(&mut self, id: &str, title: &str) {
        if self.tasks.iter().any(|task| task.id == id) {
            return;
        }
        self.tasks.push(PlanTask {
            id: id.to_owned(),
            title: title.to_owned(),
            ..PlanTask::default()
        });
    }

    fn set_state(&mut self, id: &str, state: TaskState) {
        if let Some(task) = self.task_mut(id) {
            task.state = state;
        }
    }

    fn count_check(&mut self, id: &str, passed: bool) {
        let Some(task) = self.task_mut(id) else {
            return;
        };
        let count = if passed {
            &mut task.checks_passed
        } else {
            &mut task.checks_failed
        };
        *count += 1;
    }

    fn task_mut(&mut self, id: &str) -> Option<&mut PlanTask> {
        self.tasks.iter_mut().find(|task| task.id == id)
    }
}

/// The description and scope out of an objective; an empty one reads as
/// none.
fn objective_parts(objective: &Objective) -> (Option<String>, Option<String>) {
    let filled = |text: &str| (!text.is_empty()).then(|| text.to_owned());
    (filled(&objective.description), filled(&objective.scope))
}

/// One planned task with the checks recorded on it counted by outcome.
fn task_row(task: &Task) -> PlanTask {
    let passed = task.checks.iter().filter(|check| check.passed).count();
    PlanTask {
        id: task.id.clone(),
        title: task.title.clone(),
        state: TaskState::from(task.status),
        checks_passed: u32::try_from(passed).unwrap_or(u32::MAX),
        checks_failed: u32::try_from(task.checks.len() - passed).unwrap_or(u32::MAX),
    }
}

#[cfg(test)]
#[path = "ship_view.tests.rs"]
mod tests;
