//! Pure renderers for the think-and-ship roadmap as `magistr roadmap` shows
//! it. No colour, no IO — the command layer decides where the text goes.

use magistr_engine::NativeRoadmapSnapshot;
use magistr_engine::native::{Chunk, ChunkStatus};

/// What `next`/`status` say on a project that still runs the YAML roadmap.
pub(crate) const NATIVE_REQUIRED: &str = "This project's roadmap backend is `yaml`. Set \
     \"roadmap_backend\": \"think-and-ship\" in .magistr/project.json to read the \
     think-and-ship roadmap (`magistr roadmap show` and `export` keep working on YAML).";

/// The human `show` view: current workstreams, history, the next ready chunk, and the
/// pending chunks in priority order.
pub(crate) fn render_show(snapshot: &NativeRoadmapSnapshot, chunks: &[Chunk]) -> String {
    let mut out = String::new();
    let progress = &snapshot.progress;
    let pct = progress.completion() * 100.0;
    out.push_str(&format!(
        "Roadmap (think-and-ship · {}): {}/{} chunks done ({pct:.0}%)\n",
        snapshot.project_id, progress.checked_tasks, progress.total_tasks
    ));
    out.push_str(&format!(
        "  pending {} · in progress {} · blocked {} · backlog {} · obsoleted {}\n",
        snapshot.count("pending"),
        snapshot.count("in_progress"),
        snapshot.count("blocked"),
        snapshot.count("backlog"),
        snapshot.count("obsoleted"),
    ));

    if !snapshot.workstreams.current.is_empty() {
        out.push_str("\n  Current workstreams:\n");
        for (name, work) in &snapshot.workstreams.current {
            out.push_str(&format!(
                "    {name}: {} unfinished · {} ready · {} waiting · {} blocked\n",
                work.counts.total,
                work.readiness.ready,
                work.readiness.waiting,
                work.readiness.blocked,
            ));
        }
    }
    let ungrouped = &snapshot.workstreams.ungrouped;
    if ungrouped.counts.total > 0 {
        out.push_str(&format!(
            "\n  Ungrouped work: {} unfinished · {} ready · {} waiting · {} blocked\n",
            ungrouped.counts.total,
            ungrouped.readiness.ready,
            ungrouped.readiness.waiting,
            ungrouped.readiness.blocked,
        ));
    }
    if !snapshot.workstreams.history.is_empty()
        || snapshot.workstreams.ungrouped_history.done
            + snapshot.workstreams.ungrouped_history.obsoleted
            > 0
    {
        out.push_str("\n  History:\n");
        for (name, history) in &snapshot.workstreams.history {
            out.push_str(&format!(
                "    {name}: {} done · {} obsoleted\n",
                history.done, history.obsoleted,
            ));
        }
        let history = &snapshot.workstreams.ungrouped_history;
        if history.done + history.obsoleted > 0 {
            out.push_str(&format!(
                "    Ungrouped: {} done · {} obsoleted\n",
                history.done, history.obsoleted,
            ));
        }
    }

    match snapshot
        .next_ready
        .as_deref()
        .and_then(|id| chunks.iter().find(|c| c.id == id))
    {
        Some(next) => out.push_str(&format!("\n  Next ready: {} — {}\n", next.id, next.title)),
        None => {
            out.push_str("\n  Next ready: none (every pending chunk waits on deps or a blocker)\n")
        }
    }

    let mut pending: Vec<&Chunk> = chunks
        .iter()
        .filter(|c| c.status == ChunkStatus::Pending)
        .collect();
    pending.sort_by_key(|c| (c.priority, c.id.clone()));
    if !pending.is_empty() {
        out.push_str("\n  Pending (priority order):\n");
        for (i, c) in pending.iter().take(10).enumerate() {
            let blocked = if c.blocked_by.is_some() {
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
        if pending.len() > 10 {
            out.push_str(&format!("      … and {} more\n", pending.len() - 10));
        }
    } else if progress.total_tasks > 0 {
        out.push_str("\n  Nothing pending.\n");
    }
    out
}

/// The human `next` view: the chunk to implement, with its acceptance.
pub(crate) fn render_next(next: Option<&Chunk>) -> String {
    let Some(c) = next else {
        return "No chunk is ready: every pending chunk waits on a dependency or carries a blocker.\n".to_owned();
    };
    let mut out = format!("{} — {}\n", c.id, c.title);
    if let Some(group) = &c.group {
        out.push_str(&format!("  workstream: {group}\n"));
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
pub(crate) fn show_json(snapshot: &NativeRoadmapSnapshot, chunks: &[Chunk]) -> serde_json::Value {
    let pending: Vec<serde_json::Value> = {
        let mut p: Vec<&Chunk> = chunks
            .iter()
            .filter(|c| c.status == ChunkStatus::Pending)
            .collect();
        p.sort_by_key(|c| (c.priority, c.id.clone()));
        p.iter()
            .map(|c| {
                serde_json::json!({
                    "id": c.id,
                    "title": c.title,
                    "priority": c.priority,
                    "group": c.group,
                    "deps": c.deps,
                    "blocked": c.blocked_by.is_some(),
                })
            })
            .collect()
    };
    serde_json::json!({
        "backend": "think-and-ship",
        "project_id": snapshot.project_id,
        "counts": snapshot.counts,
        "workstreams": snapshot.workstreams,
        "next_ready": snapshot.next_ready,
        "in_progress": snapshot.in_progress,
        "progress": snapshot.progress,
        "pending": pending,
    })
}

#[cfg(test)]
#[path = "roadmap_native_render.tests.rs"]
mod tests;
