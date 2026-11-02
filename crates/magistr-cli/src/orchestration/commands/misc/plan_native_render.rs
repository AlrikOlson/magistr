//! Pure renderers for the magistr plan as `magistr plan` shows
//! it. No colour, no IO — the command layer decides where the text goes.

use magistr_engine::NativePlanSnapshot;
use magistr_engine::native::{Task, TaskStatus};

/// What `next`/`status` say on a project that still runs the YAML plan.
pub(crate) const NATIVE_REQUIRED: &str = "This project's plan backend is `yaml`. Set \
     \"plan_backend\": \"native\" in .magistr/project.json to read the \
     magistr plan (`magistr plan show` and `export` keep working on YAML).";

/// The human `show` view: current features, history, the next ready task, and the
/// to-do tasks in priority order.
pub(crate) fn render_show(snapshot: &NativePlanSnapshot, tasks: &[Task]) -> String {
    let mut out = String::new();
    let progress = &snapshot.progress;
    let pct = progress.completion() * 100.0;
    out.push_str(&format!(
        "Plan (magistr · {}): {}/{} tasks done ({pct:.0}%)\n",
        snapshot.project_id, progress.checked_tasks, progress.total_tasks
    ));
    out.push_str(&format!(
        "  to do {} · in progress {} · waiting {} · ideas {} · retired {}\n",
        snapshot.count("to_do"),
        snapshot.count("in_progress"),
        snapshot.count("waiting"),
        snapshot.count("idea"),
        snapshot.count("retired"),
    ));

    if !snapshot.features.current.is_empty() {
        out.push_str("\n  Current features:\n");
        for (name, work) in &snapshot.features.current {
            out.push_str(&format!(
                "    {name}: {} unfinished · {} ready · {} after other tasks · {} waiting\n",
                work.counts.total,
                work.readiness.ready,
                work.readiness.dependent,
                work.readiness.waiting,
            ));
        }
    }
    let unassigned = &snapshot.features.unassigned;
    if unassigned.counts.total > 0 {
        out.push_str(&format!(
            "\n  Unassigned work: {} unfinished · {} ready · {} after other tasks · {} waiting\n",
            unassigned.counts.total,
            unassigned.readiness.ready,
            unassigned.readiness.dependent,
            unassigned.readiness.waiting,
        ));
    }
    if !snapshot.features.history.is_empty()
        || snapshot.features.unassigned_history.done + snapshot.features.unassigned_history.retired
            > 0
    {
        out.push_str("\n  History:\n");
        for (name, history) in &snapshot.features.history {
            out.push_str(&format!(
                "    {name}: {} done · {} retired\n",
                history.done, history.retired,
            ));
        }
        let history = &snapshot.features.unassigned_history;
        if history.done + history.retired > 0 {
            out.push_str(&format!(
                "    Unassigned: {} done · {} retired\n",
                history.done, history.retired,
            ));
        }
    }

    match snapshot
        .next_ready
        .as_deref()
        .and_then(|id| tasks.iter().find(|c| c.id == id))
    {
        Some(next) => out.push_str(&format!("\n  Next ready: {} — {}\n", next.id, next.title)),
        None => out.push_str(
            "\n  Next ready: none (every to-do task waits on other tasks or a named reason)\n",
        ),
    }

    let mut to_do: Vec<&Task> = tasks
        .iter()
        .filter(|c| c.status == TaskStatus::ToDo)
        .collect();
    to_do.sort_by_key(|c| (c.priority, c.id.clone()));
    if !to_do.is_empty() {
        out.push_str("\n  Pending (priority order):\n");
        for (i, c) in to_do.iter().take(10).enumerate() {
            let blocked = if c.waiting_on.is_some() {
                "  [blocked]"
            } else {
                ""
            };
            out.push_str(&format!(
                "  {:>2}. [{}] {} — {}{blocked}\n",
                i + 1,
                c.priority,
                c.id,
                c.title
            ));
        }
        if to_do.len() > 10 {
            out.push_str(&format!("      … and {} more\n", to_do.len() - 10));
        }
    } else if progress.total_tasks > 0 {
        out.push_str("\n  Nothing to_do.\n");
    }
    out
}

/// The human `next` view: the task to implement, with its acceptance.
pub(crate) fn render_next(next: Option<&Task>) -> String {
    let Some(c) = next else {
        return "No task is ready: every to_do task waits on a dependency or carries a blocker.\n"
            .to_owned();
    };
    let mut out = format!("{} — {}\n", c.id, c.title);
    if let Some(feature) = &c.feature {
        out.push_str(&format!("  feature: {feature}\n"));
    }
    out.push_str(&format!("  priority: {}\n", c.priority));
    if !c.deps.is_empty() {
        out.push_str(&format!("  deps: {}\n", c.deps.join(", ")));
    }
    if !c.description.trim().is_empty() {
        out.push_str(&format!("\n  {}\n", c.description.trim()));
    }
    if !c.acceptance.is_empty() {
        out.push_str("\n  Acceptance:\n");
        for a in &c.acceptance {
            out.push_str(&format!("  - {a}\n"));
        }
    }
    out
}

/// The machine `show` view.
pub(crate) fn show_json(snapshot: &NativePlanSnapshot, tasks: &[Task]) -> serde_json::Value {
    let to_do: Vec<serde_json::Value> = {
        let mut p: Vec<&Task> = tasks
            .iter()
            .filter(|c| c.status == TaskStatus::ToDo)
            .collect();
        p.sort_by_key(|c| (c.priority, c.id.clone()));
        p.iter()
            .map(|c| {
                serde_json::json!({
                    "id": c.id,
                    "title": c.title,
                    "priority": c.priority,
                    "feature": c.feature,
                    "deps": c.deps,
                    "waiting": c.waiting_on.is_some(),
                })
            })
            .collect()
    };
    serde_json::json!({
        "backend": "magistr",
        "project_id": snapshot.project_id,
        "counts": snapshot.counts,
        "features": snapshot.features,
        "next_ready": snapshot.next_ready,
        "in_progress": snapshot.in_progress,
        "progress": snapshot.progress,
        "to_do": to_do,
    })
}

#[cfg(test)]
#[path = "plan_native_render.tests.rs"]
mod tests;
