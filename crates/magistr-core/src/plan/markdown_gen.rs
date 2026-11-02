//! Markdown generator for the plan.
//!
//! Renders a [`Plan`] struct into a pretty `PLAN.md` string.
//! Renders the current YAML plan as an explicit Markdown export.

use super::phase::Phase;
use super::schema::Plan;
use super::task::Task;

/// Render a plan as a markdown string.
///
/// The output describes the plan's phases and tasks for an explicit export.
#[must_use]
pub fn render(plan: &Plan) -> String {
    let mut out = String::new();

    render_header(plan, &mut out);

    for (i, phase) in plan.phases.iter().enumerate() {
        if i > 0 || !plan.preamble.is_empty() || !plan.vision.is_empty() {
            out.push_str("---\n\n");
        }
        render_phase(phase, &mut out);
    }

    if !plan.epilogue.is_empty() {
        out.push_str("---\n\n");
        out.push_str(&plan.epilogue);
        out.push('\n');
    }

    out
}

/// Render title, preamble, vision, and principles.
fn render_header(plan: &Plan, out: &mut String) {
    out.push_str("# ");
    out.push_str(&plan.title);
    out.push_str("\n\n");

    if !plan.preamble.is_empty() {
        out.push_str(&plan.preamble);
        out.push_str("\n\n");
    }

    if !plan.vision.is_empty() {
        out.push_str("**Vision:** ");
        out.push_str(&plan.vision);
        out.push_str("\n\n");
    }

    if !plan.principles.is_empty() {
        out.push_str("**Principles:**\n");
        for p in &plan.principles {
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
