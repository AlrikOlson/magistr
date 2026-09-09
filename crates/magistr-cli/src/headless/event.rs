//! Headless event types emitted during CI/headless sessions.

use serde::Serialize;

/// Events emitted during a headless session.
///
/// Each renderer formats these events differently:
/// - `JsonStreamRenderer` → NDJSON lines
/// - `TextSummaryRenderer` → human-readable text
/// - `GhaRenderer` → GitHub Actions annotations
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data", rename_all = "snake_case")]
pub enum HeadlessEvent {
    /// A quality gate is starting.
    GateStart { name: String },
    /// A quality gate passed.
    GatePass { name: String, duration_ms: u64 },
    /// A quality gate failed.
    GateFail {
        name: String,
        duration_ms: u64,
        output: Option<String>,
    },
    /// A session started.
    SessionStart { session_id: String },
    /// A session ended.
    SessionEnd {
        duration_ms: u64,
        cost_usd: Option<f64>,
        exit_code: u8,
    },
    /// An iteration started (next-loop mode).
    IterationStart { iteration: u32 },
    /// An iteration completed (next-loop mode).
    IterationEnd {
        iteration: u32,
        duration_ms: u64,
        tasks_completed: u32,
    },
    /// A runtime error occurred.
    Error { message: String },
    /// The spending budget was exceeded.
    BudgetExceeded { limit_usd: f64, spent_usd: f64 },
}

#[cfg(test)]
#[path = "event.tests.rs"]
mod tests;
