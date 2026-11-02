use magistr_workflow::plan::domain::{Task, TaskStatus};

use crate::plan::PlanProgress;

/// Project a magistr plan onto the engine's progress snapshot.
///
/// A task is a task; a feature (`feature`) is a phase. Retired tasks
/// are history and count toward nothing — the plan that carries 73 of
/// them would otherwise read as one-third done before any work.
#[must_use]
pub fn progress_from_tasks(tasks: &[Task]) -> PlanProgress {
    let live = tasks.iter().filter(|c| c.status != TaskStatus::Retired);
    let mut total_tasks = 0u32;
    let mut checked_tasks = 0u32;
    // feature → (live, done)
    let mut features: std::collections::BTreeMap<&str, (u32, u32)> = Default::default();
    for task in live {
        total_tasks += 1;
        let done = task.status == TaskStatus::Done;
        if done {
            checked_tasks += 1;
        }
        if let Some(feature) = task.feature.as_deref() {
            let entry = features.entry(feature).or_insert((0, 0));
            entry.0 += 1;
            if done {
                entry.1 += 1;
            }
        }
    }
    let total_phases = u32::try_from(features.len()).unwrap_or(u32::MAX);
    let completed_phases = features
        .values()
        .filter(|(live, done)| live == done)
        .count();
    PlanProgress {
        total_tasks,
        checked_tasks,
        total_phases,
        completed_phases: u32::try_from(completed_phases).unwrap_or(u32::MAX),
    }
}

#[cfg(test)]
#[path = "progress.tests.rs"]
mod tests;
