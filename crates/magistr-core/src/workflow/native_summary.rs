//! The magistr plan as a [`SpecSummary`], so every consumer of
//! spec progress (the iteration loop, the workflow config, the prompt
//! context) reads tasks on a native project and never opens `spec.file`.

use magistr_engine::NativeReader;
use magistr_engine::native::{
    FocusState, GoalStatus, Readiness, Task, TaskStatus, WorkstreamResolution, assess_readiness,
    next_ready, progress_from_tasks, workstream_catalog,
};

use super::spec::SpecSummary;
use crate::error::{MagistrError, Result};

/// Progress counts all unfinished tasks; the item list selects the next
/// ready task for the prompt. Availability never means completion.
/// Re-reads the plan because the admitted writer may update it between turns.
pub fn native_spec_summary(engines: &NativeReader) -> Result<SpecSummary> {
    let tasks = engines.plan_tasks().map_err(MagistrError::Workflow)?;
    let progress = progress_from_tasks(&tasks);
    let unchecked_items: Vec<String> = next_ready(
        &tasks,
        |task| {
            task.readiness(|id| tasks.iter().find(|c| c.id == id).map(|c| c.status))
                .is_ready()
        },
        |task| task.priority,
    )
    .map(|c| vec![format!("{} — {}", c.id, c.title)])
    .unwrap_or_default();
    let unchecked = progress.total_tasks.saturating_sub(progress.checked_tasks) as usize;
    Ok(SpecSummary {
        total: progress.total_tasks as usize,
        checked: progress.checked_tasks as usize,
        unchecked,
        unchecked_items,
    })
}

/// Select work for an admitted loop without changing plan or goal state.
/// A saved target and an unfinished goal take precedence over ambiguity.
pub(crate) fn native_work_task(
    engines: &NativeReader,
    preferred: Option<&str>,
    track: Option<&str>,
) -> Result<Option<Task>> {
    let tasks = engines.plan_tasks().map_err(MagistrError::Workflow)?;
    select_work_task(engines, preferred, track, &tasks)
}

fn select_work_task(
    engines: &NativeReader,
    preferred: Option<&str>,
    track: Option<&str>,
    tasks: &[Task],
) -> Result<Option<Task>> {
    if let Some(track) = track {
        let catalog = workstream_catalog(tasks);
        let refusal = match catalog.focus_state(Some(track)) {
            FocusState::Current { .. } => None,
            FocusState::Closed { feature } => Some(WorkstreamResolution::Closed {
                feature,
                choices: catalog.current_names(),
            }),
            FocusState::Missing { .. } | FocusState::Unset => Some(WorkstreamResolution::Unknown {
                choices: catalog.current_names(),
            }),
        };
        if let Some(refusal) = refusal {
            return Err(MagistrError::Workflow(refusal.explain(track)));
        }
    }
    let in_track = |task: &&Task| track.is_none_or(|track| task.feature.as_deref() == Some(track));
    let goal = engines
        .run_goal()
        .map_err(MagistrError::Workflow)?
        .filter(|goal| {
            let settled_task = goal
                .scope
                .trim()
                .strip_prefix("task:")
                .and_then(|id| tasks.iter().find(|task| task.id == id.trim()))
                .is_some_and(|task| matches!(task.status, TaskStatus::Done | TaskStatus::Retired));
            matches!(goal.status, GoalStatus::Open | GoalStatus::Active) && !settled_task
        });
    let saved = preferred.and_then(|id| tasks.iter().find(|task| task.id == id));
    if preferred.is_some() && saved.is_none() {
        return Err(MagistrError::Workflow(
            "The work saved for this run is no longer in the plan. Restore it before resuming this run."
                .into(),
        ));
    }
    if let Some(task) = saved
        && matches!(task.status, TaskStatus::ToDo | TaskStatus::InProgress)
        && !in_track(&task)
    {
        return Err(MagistrError::Workflow(
            "The saved work belongs to another track. Resume that run before choosing a different track.".into(),
        ));
    }
    if let Some(task) = saved
        && matches!(task.status, TaskStatus::Waiting | TaskStatus::Idea)
    {
        return Err(MagistrError::Workflow(format!(
            "Cannot resume “{}”: this work is {}.",
            work_name(task),
            if task.status == TaskStatus::Waiting {
                "waiting"
            } else {
                "not scheduled"
            },
        )));
    }
    let saved =
        saved.filter(|task| matches!(task.status, TaskStatus::ToDo | TaskStatus::InProgress));
    let goal_task = goal
        .as_ref()
        .and_then(|goal| goal.scope.trim().strip_prefix("task:"))
        .and_then(|id| tasks.iter().find(|task| task.id == id.trim()))
        .filter(|task| task.status == TaskStatus::InProgress)
        .filter(in_track);
    let selected = match saved.or(goal_task) {
        Some(task) => Some(task.clone()),
        None => {
            let active: Vec<_> = tasks
                .iter()
                .filter(|task| task.status == TaskStatus::InProgress)
                .filter(in_track)
                .collect();
            match active.as_slice() {
                [] => next_ready(
                    tasks.iter().filter(in_track),
                    |task| {
                        task.readiness(|id| tasks.iter().find(|c| c.id == id).map(|c| c.status))
                            .is_ready()
                    },
                    |task| task.priority,
                )
                .cloned(),
                [task] => Some((*task).clone()),
                _ => {
                    return Err(MagistrError::Workflow(format!(
                        "More than one work item is in progress: {}. Choose which work to resume.",
                        active
                            .iter()
                            .map(|task| format!("“{}”", work_name(task)))
                            .collect::<Vec<_>>()
                            .join(", "),
                    )));
                }
            }
        }
    };
    let Some(task) = selected else {
        return Ok(None);
    };
    ensure_available(&task, tasks)?;
    Ok(Some(task))
}

fn ensure_available(task: &Task, tasks: &[Task]) -> Result<()> {
    // The caller has selected pending or in-progress work. Resuming either
    // requires the same current blockers and dependencies as scheduling it.
    match assess_readiness(
        TaskStatus::ToDo,
        task.waiting_on
            .as_ref()
            .map(|blocker| blocker.reason.as_str()),
        &task.deps,
        |id| {
            tasks
                .iter()
                .find(|candidate| candidate.id == id)
                .map(|candidate| candidate.status)
        },
    ) {
        Readiness::Waiting(reason) => Err(MagistrError::Workflow(format!(
            "Cannot resume “{}”: {reason}",
            work_name(task),
        ))),
        Readiness::Dependent(dependencies) => {
            let dependency = dependencies.first().and_then(|dependency| {
                tasks.iter().find(|candidate| candidate.id == dependency.id)
            });
            Err(MagistrError::Workflow(match dependency {
                Some(dependency) => format!(
                    "Cannot resume “{}” until “{}” is complete.",
                    work_name(task),
                    work_name(dependency),
                ),
                None => format!(
                    "Cannot resume “{}”: a required work item is missing from the plan.",
                    work_name(task),
                ),
            }))
        }
        Readiness::Ready | Readiness::Inactive(_) => Ok(()),
    }
}

fn work_name(task: &Task) -> &str {
    if !task.title.trim().is_empty() {
        task.title.trim()
    } else if !task.name.trim().is_empty() {
        task.name.trim()
    } else {
        "Untitled work item"
    }
}

/// Actionable progress for a loop. General reports retain `native_spec_summary`.
pub(crate) fn native_work_summary(
    engines: &NativeReader,
    preferred: Option<&str>,
    track: Option<&str>,
) -> Result<SpecSummary> {
    let tasks = engines.plan_tasks().map_err(MagistrError::Workflow)?;
    let task = select_work_task(engines, preferred, track, &tasks)?;
    let progress = progress_from_tasks(&tasks);
    let unchecked_items: Vec<_> = task
        .into_iter()
        .map(|task| {
            let suffix = if task.status == TaskStatus::InProgress {
                " (in progress; resume this work)"
            } else {
                ""
            };
            format!("{} — {}{suffix}", task.id, task.title)
        })
        .collect();
    Ok(SpecSummary {
        total: progress.total_tasks as usize,
        checked: progress.checked_tasks as usize,
        unchecked: progress.total_tasks.saturating_sub(progress.checked_tasks) as usize,
        unchecked_items,
    })
}

#[cfg(test)]
#[path = "native_summary.tests.rs"]
mod tests;
