//! Interview trace logger — comprehensive observability for AI interview sessions.
//!
//! Writes a structured NDJSON log to `.magistr/interview-trace.log` capturing every
//! message, tool call, state transition, and timing detail. Designed for
//! post-session diagnosis of stalls, timeouts, and unexpected AI behavior.
//!
//! # Log Format
//!
//! Each line is a JSON object with at minimum:
//! - `ts`: ISO 8601 timestamp
//! - `elapsed_ms`: milliseconds since session start
//! - `event`: event type tag
//! - `data`: event-specific payload
//!
//! # Usage
//!
//! ```ignore
//! let tracer = InterviewTracer::new(&project_dir);
//! tracer.log_session_start(&config);
//! tracer.log_message_received("assistant", &summary);
//! tracer.log_tool_call("magistr_ask", &input_json);
//! // ... after session:
//! // cat .magistr/interview-trace.log | jq .
//! ```

use serde_json::json;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Shared inner state — behind `Arc` so the tracer is cheaply cloneable.
struct TracerInner {
    file: Mutex<Option<std::fs::File>>,
    start: Instant,
    log_path: PathBuf,
}

/// Trace logger for interview sessions.
///
/// Thread-safe and `Clone`-able (Arc-wrapped internals). All methods are
/// infallible — logging failures are silently ignored to never disrupt the
/// interview. Cheap to clone for passing into spawned tasks.
#[derive(Clone)]
pub struct InterviewTracer {
    inner: Arc<TracerInner>,
}

impl InterviewTracer {
    /// Create a new tracer that writes to `.magistr/interview-trace.log` in the project dir.
    ///
    /// Creates the `.magistr/` directory if needed. If file creation fails,
    /// the tracer becomes a no-op.
    pub fn new(project_dir: &Path) -> Self {
        let magistr_dir = crate::paths::magistr_dir(project_dir);
        let _ = std::fs::create_dir_all(&magistr_dir);
        let log_path = crate::paths::interview_trace(project_dir);

        // Truncate any previous trace file for a clean session
        let file = std::fs::File::create(&log_path).ok();

        Self {
            inner: Arc::new(TracerInner {
                file: Mutex::new(file),
                start: Instant::now(),
                log_path,
            }),
        }
    }

    /// Path to the trace log file.
    pub fn log_path(&self) -> &Path {
        &self.inner.log_path
    }

    /// Write a trace event. Infallible — errors are silently dropped.
    fn write_event(&self, event: &str, data: serde_json::Value) {
        let elapsed = self.inner.start.elapsed();
        let record = json!({
            "ts": chrono_now(),
            "elapsed_ms": elapsed.as_millis() as u64,
            "event": event,
            "data": data,
        });
        if let Ok(mut guard) = self.inner.file.lock()
            && let Some(ref mut f) = *guard
        {
            let _ = writeln!(f, "{}", record);
            let _ = f.flush();
        }
    }

    // ── Session lifecycle ───────────────────────────────────────

    /// Log session start with configuration details.
    pub fn log_session_start(
        &self,
        model: &str,
        max_turns: usize,
        allowed_tools: &[String],
        prompt_len: usize,
    ) {
        self.write_event(
            "session_start",
            json!({
                "model": model,
                "max_turns": max_turns,
                "allowed_tools": allowed_tools,
                "prompt_length_bytes": prompt_len,
            }),
        );
    }

    /// Log session start failure.
    pub fn log_session_start_failed(&self, error: &str) {
        self.write_event("session_start_failed", json!({ "error": error }));
    }

    /// Log session completion.
    pub fn log_session_complete(&self, reason: &str) {
        self.write_event("session_complete", json!({ "reason": reason }));
    }

    // ── Message flow ────────────────────────────────────────────

    /// Log a raw ACP message received from the agent.
    pub fn log_message_received(&self, variant: &str, summary: &str) {
        self.write_event(
            "message_received",
            json!({
                "variant": variant,
                "summary": truncate(summary, 500),
            }),
        );
    }

    /// Log assistant text content (the actual AI response text).
    pub fn log_assistant_text(&self, text: &str) {
        self.write_event(
            "assistant_text",
            json!({
                "length": text.len(),
                "preview": truncate(text, 300),
            }),
        );
    }

    /// Log a tool use (AI calling a tool).
    pub fn log_tool_use(&self, tool_name: &str, tool_id: &str, input: &serde_json::Value) {
        self.write_event(
            "tool_use",
            json!({
                "tool": tool_name,
                "id": tool_id,
                "input_preview": truncate(&input.to_string(), 500),
            }),
        );
    }

    /// Log tool extraction result (what magistr extracted from the tool call).
    pub fn log_tool_extraction(&self, tool_name: &str, result: &str) {
        self.write_event(
            "tool_extraction",
            json!({
                "tool": tool_name,
                "result": result,
            }),
        );
    }

    /// Log a tool result returned to the AI.
    pub fn log_tool_result(&self, tool_name: &str, result_preview: &str) {
        self.write_event(
            "tool_result",
            json!({
                "tool": tool_name,
                "result_preview": truncate(result_preview, 300),
            }),
        );
    }

    // ── Turn management ─────────────────────────────────────────

    /// Log turn start.
    pub fn log_turn_start(&self, turn_number: usize) {
        self.write_event("turn_start", json!({ "turn": turn_number }));
    }

    /// Log turn result (end of a turn).
    pub fn log_turn_result(
        &self,
        turn_number: usize,
        duration_ms: Option<f64>,
        cost_usd: Option<f64>,
        num_turns: Option<u32>,
    ) {
        self.write_event(
            "turn_result",
            json!({
                "turn": turn_number,
                "duration_ms": duration_ms,
                "cost_usd": cost_usd,
                "num_turns_reported": num_turns,
            }),
        );
    }

    // ── Interview state ─────────────────────────────────────────

    /// Log state transition in the interview loop.
    pub fn log_state_transition(&self, from: &str, to: &str, reason: &str) {
        self.write_event(
            "state_transition",
            json!({
                "from": from,
                "to": to,
                "reason": reason,
            }),
        );
    }

    /// Log that a magistr_ask question was extracted.
    pub fn log_question_extracted(&self, category: &str, num_choices: usize) {
        self.write_event(
            "question_extracted",
            json!({
                "category": category,
                "num_choices": num_choices,
            }),
        );
    }

    /// Log that magistr_recommend was extracted.
    pub fn log_recommendations_extracted(&self, summary: &str) {
        self.write_event("recommendations_extracted", json!({ "summary": summary }));
    }

    /// Log user response being sent as followup.
    pub fn log_followup_sent(&self, content: &str) {
        self.write_event(
            "followup_sent",
            json!({
                "content": truncate(content, 300),
                "length": content.len(),
            }),
        );
    }

    /// Log idle timeout.
    pub fn log_idle_timeout(&self, seconds: u64, had_recommendations: bool) {
        self.write_event(
            "idle_timeout",
            json!({
                "timeout_seconds": seconds,
                "had_recommendations": had_recommendations,
            }),
        );
    }

    // ── Errors and edge cases ───────────────────────────────────

    /// Log an error or unexpected condition.
    pub fn log_error(&self, context: &str, error: &str) {
        self.write_event(
            "error",
            json!({
                "context": context,
                "error": error,
            }),
        );
    }

    /// Log a stream event (tool progress).
    pub fn log_stream_event(&self, event_preview: &str) {
        self.write_event(
            "stream_event",
            json!({ "preview": truncate(event_preview, 200) }),
        );
    }

    /// Log thinking block from the AI.
    pub fn log_thinking(&self, length: usize) {
        self.write_event("thinking", json!({ "length": length }));
    }

    /// Log MCP config details.
    pub fn log_mcp_config(&self, config: &serde_json::Value) {
        self.write_event("mcp_config", json!({ "config": config }));
    }

    /// Log the full prompt being sent (first N chars).
    pub fn log_prompt(&self, prompt: &str) {
        self.write_event(
            "prompt",
            json!({
                "length": prompt.len(),
                "preview": truncate(prompt, 1000),
            }),
        );
    }

    // ── Permission handling ─────────────────────────────────────

    /// Log an auto-approved permission request.
    pub fn log_permission_auto_approved(&self, tool_title: &str, option_id: &str) {
        self.write_event(
            "permission_auto_approved",
            json!({
                "tool": tool_title,
                "option_id": option_id,
            }),
        );
    }
}

/// Get current time as ISO 8601 string without pulling in chrono.
fn chrono_now() -> String {
    // Use SystemTime → format manually
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    // Simple UTC timestamp: seconds since epoch (readable enough for debugging)
    // For full ISO 8601 we'd need chrono, but this is fine for trace logs
    format!("{secs}")
}

/// Truncate a string to at most `max_len` characters, appending "…" if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    crate::utils::text::truncate(s, max_len)
}

#[cfg(test)]
#[path = "trace.tests.rs"]
mod tests;
