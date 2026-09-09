//! Plain text summary renderer for headless mode.
//!
//! Produces human-readable CI logs without ANSI escape codes.
//! Suitable for build logs, redirected output, and terminals without color support.

use std::io::Write;

use super::event::HeadlessEvent;
use super::renderer::HeadlessRenderer;

/// Human-readable plain text renderer for CI logs.
///
/// Output is intentionally free of ANSI escape codes so it renders
/// cleanly in CI build logs, log aggregators, and plain text viewers.
#[derive(Debug, Default)]
pub struct TextSummaryRenderer;

impl TextSummaryRenderer {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

/// Format milliseconds as a human-friendly duration string.
fn format_duration(ms: u64) -> String {
    if ms < 1_000 {
        format!("{ms}ms")
    } else {
        format!("{:.1}s", ms as f64 / 1_000.0)
    }
}

impl HeadlessRenderer for TextSummaryRenderer {
    fn emit(&self, w: &mut dyn Write, event: HeadlessEvent) -> std::io::Result<()> {
        match event {
            HeadlessEvent::GateStart { name } => {
                writeln!(w, "--- {name} ---")
            }
            HeadlessEvent::GatePass { name, duration_ms } => {
                writeln!(
                    w,
                    "  [PASS] {name} ({duration})",
                    duration = format_duration(duration_ms)
                )
            }
            HeadlessEvent::GateFail {
                name,
                duration_ms,
                output,
            } => {
                writeln!(
                    w,
                    "  [FAIL] {name} ({duration})",
                    duration = format_duration(duration_ms)
                )?;
                if let Some(out) = output {
                    for line in out.lines() {
                        writeln!(w, "    {line}")?;
                    }
                }
                Ok(())
            }
            HeadlessEvent::SessionStart { session_id } => {
                writeln!(w, "Session started: {session_id}")
            }
            HeadlessEvent::SessionEnd {
                duration_ms,
                cost_usd,
                exit_code,
            } => {
                write!(
                    w,
                    "Session ended: {duration}",
                    duration = format_duration(duration_ms),
                )?;
                if let Some(cost) = cost_usd {
                    write!(w, ", cost: ${cost:.2}")?;
                }
                writeln!(w, ", exit: {exit_code}")
            }
            HeadlessEvent::IterationStart { iteration } => {
                writeln!(w, "=== Iteration {iteration} ===")
            }
            HeadlessEvent::IterationEnd {
                iteration,
                duration_ms,
                tasks_completed,
            } => {
                writeln!(
                    w,
                    "  Iteration {iteration} done: {tasks_completed} tasks, {duration}",
                    duration = format_duration(duration_ms),
                )
            }
            HeadlessEvent::Error { message } => {
                writeln!(w, "ERROR: {message}")
            }
            HeadlessEvent::BudgetExceeded {
                limit_usd,
                spent_usd,
            } => {
                writeln!(
                    w,
                    "WARNING: budget exceeded (limit: ${limit_usd:.2}, spent: ${spent_usd:.2})"
                )
            }
        }
    }
}

#[cfg(test)]
#[path = "text_summary.tests.rs"]
mod tests;
