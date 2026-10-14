//! Markdown generator for the roadmap.
//!
//! Renders a [`Roadmap`] struct into a pretty `ROADMAP.md` string.
//! Renders the current YAML roadmap as an explicit Markdown export.

use super::phase::Phase;
use super::schema::Roadmap;
use super::task::Task;

/// Render a roadmap as a markdown string.
///
/// The output describes the roadmap's phases and tasks for an explicit export.
#[must_use]
pub fn render(roadmap: &Roadmap) -> String {
    let mut out = String::new();

    render_header(roadmap, &mut out);

    for (i, phase) in roadmap.phases.iter().enumerate() {
        if i > 0 || !roadmap.preamble.is_empty() || !roadmap.vision.is_empty() {
            out.push_str("---\n\n");
        }
        render_phase(phase, &mut out);
    }

    if !roadmap.epilogue.is_empty() {
        out.push_str("---\n\n");
        out.push_str(&roadmap.epilogue);
        out.push('\n');
    }

    out
}

/// Render title, preamble, vision, and principles.
fn render_header(roadmap: &Roadmap, out: &mut String) {
    out.push_str("# ");
    out.push_str(&roadmap.title);
    out.push_str("\n\n");

    if !roadmap.preamble.is_empty() {
        out.push_str(&roadmap.preamble);
        out.push_str("\n\n");
    }

    if !roadmap.vision.is_empty() {
        out.push_str("**Vision:** ");
        out.push_str(&roadmap.vision);
        out.push_str("\n\n");
    }

    if !roadmap.principles.is_empty() {
        out.push_str("**Principles:**\n");
        for p in &roadmap.principles {
            out.push_str("- ");
            out.push_str(p);
            out.push('\n');
        }
        out.push('\n');
    }
}

/// Render a single phase: heading, problem/solution, tasks.
fn render_phase(phase: &Phase, out: &mut String) {
    // Heading: ## Phase E0: Name ✦ "Tagline"
    out.push_str("## Phase ");
    out.push_str(&phase.id);
    out.push_str(": ");
    out.push_str(&phase.name);
    if let Some(tagline) = &phase.tagline {
        out.push_str(" ✦ \"");
        out.push_str(tagline);
        out.push('"');
    }
    out.push_str("\n\n");

    if !phase.problem.is_empty() {
        out.push_str("**Problem:** ");
        out.push_str(&phase.problem);
        out.push_str("\n\n");
    }

    if !phase.solution.is_empty() {
        out.push_str("**Solution:** ");
        out.push_str(&phase.solution);
        out.push_str("\n\n");
    }

    if !phase.tasks.is_empty() {
        out.push_str("### Tasks\n\n");
        for task in &phase.tasks {
            render_task(task, 0, out);
        }
        out.push('\n');
    }
}

/// Render a task line with its subtasks, recursively.
fn render_task(task: &Task, indent: usize, out: &mut String) {
    let prefix = " ".repeat(indent);
    let check = if task.checked { "x" } else { " " };
    out.push_str(&prefix);
    out.push_str("- [");
    out.push_str(check);
    out.push_str("] ");
    out.push_str(&task.text);
    out.push('\n');

    for sub in &task.subtasks {
        render_task(sub, indent + 2, out);
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "markdown_gen.tests.rs"]
mod tests;
