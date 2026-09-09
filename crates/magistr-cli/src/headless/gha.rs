//! GitHub Actions annotation renderer for headless mode.
//!
//! Produces workflow commands that GitHub Actions interprets as annotations:
//! - `::error::message` — error annotations on the summary and PR files view
//! - `::warning::message` — warning annotations
//! - `::notice::message` — informational annotations
//! - `::group::title` / `::endgroup::` — collapsible log groups

use std::io::Write;

use super::event::HeadlessEvent;
use super::renderer::HeadlessRenderer;

/// GitHub Actions workflow command renderer.
///
/// Formats events as GitHub Actions annotation commands so they appear
/// as inline annotations in the Actions summary and PR files view.
#[derive(Debug, Default)]
pub struct GhaRenderer;

impl GhaRenderer {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

/// Escape a message for use in a GitHub Actions workflow command.
///
/// Workflow commands use `%0A` for newlines and `%25` for literal `%`.
fn escape_gha(msg: &str) -> String {
    msg.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

impl HeadlessRenderer for GhaRenderer {
    fn emit(&self, w: &mut dyn Write, event: HeadlessEvent) -> std::io::Result<()> {
        match event {
            HeadlessEvent::GateStart { name } => {
                writeln!(w, "::group::Gate: {name}")
            }
            HeadlessEvent::GatePass { name, duration_ms } => {
                writeln!(w, "::endgroup::")?;
                writeln!(w, "::notice::Gate '{name}' passed ({duration_ms}ms)")
            }
            HeadlessEvent::GateFail {
                name,
                duration_ms,
                output,
            } => {
                writeln!(w, "::endgroup::")?;
                let detail = output.as_deref().unwrap_or("no output");
                writeln!(
                    w,
                    "::error::Gate '{name}' failed ({duration_ms}ms): {detail}",
                    detail = escape_gha(detail),
                )
            }
            HeadlessEvent::SessionStart { session_id } => {
                writeln!(w, "::group::Session {session_id}")
            }
            HeadlessEvent::SessionEnd {
                duration_ms,
                cost_usd,
                exit_code,
            } => {
                writeln!(w, "::endgroup::")?;
                let cost_str = cost_usd
                    .map(|c| format!(", cost: ${c:.2}"))
                    .unwrap_or_default();
                writeln!(
                    w,
                    "::notice::Session ended ({duration_ms}ms{cost_str}, exit: {exit_code})"
                )
            }
            HeadlessEvent::IterationStart { iteration } => {
                writeln!(w, "::group::Iteration {iteration}")
            }
            HeadlessEvent::IterationEnd {
                iteration,
                duration_ms,
                tasks_completed,
            } => {
                writeln!(w, "::endgroup::")?;
                writeln!(
                    w,
                    "::notice::Iteration {iteration} done: {tasks_completed} tasks ({duration_ms}ms)"
                )
            }
            HeadlessEvent::Error { message } => {
                writeln!(w, "::error::{msg}", msg = escape_gha(&message))
            }
            HeadlessEvent::BudgetExceeded {
                limit_usd,
                spent_usd,
            } => {
                writeln!(
                    w,
                    "::warning::Budget exceeded (limit: ${limit_usd:.2}, spent: ${spent_usd:.2})"
                )
            }
        }
    }
}

#[cfg(test)]
#[path = "gha.tests.rs"]
mod tests;
