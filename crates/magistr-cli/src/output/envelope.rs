//! JSON envelope types for machine-readable output.
//!
//! Provides a standard envelope format:
//! `{ "status": "ok"|"error", "data": {...} }` or `{ "status": "error", "error": "..." }`.

use serde::Serialize;

/// Standard JSON envelope for command output.
#[derive(Debug, Serialize)]
pub struct Envelope<T: Serialize> {
    pub status: &'static str,
    pub data: T,
}

/// Error envelope when a command fails.
#[derive(Debug, Serialize)]
pub struct ErrorEnvelope {
    pub status: &'static str,
    pub error: String,
}

/// Wrap data in a success envelope.
pub fn success<T: Serialize>(data: &T) -> Envelope<&T> {
    Envelope { status: "ok", data }
}

/// Create an error envelope.
pub fn error(message: impl Into<String>) -> ErrorEnvelope {
    ErrorEnvelope {
        status: "error",
        error: message.into(),
    }
}

/// Serializable validation output for `--json` mode.
#[derive(Debug, Serialize)]
pub struct ValidationOutput {
    pub all_passed: bool,
    pub gates: Vec<GateOutput>,
    pub total_duration_ms: u64,
}

/// A single gate result for JSON output.
#[derive(Debug, Serialize)]
pub struct GateOutput {
    pub name: String,
    pub passed: bool,
    pub duration_ms: u64,
    pub command: String,
}

impl From<&crate::validate::ValidationResult> for ValidationOutput {
    fn from(result: &crate::validate::ValidationResult) -> Self {
        Self {
            all_passed: result.all_passed(),
            gates: result
                .results
                .iter()
                .map(|g| GateOutput {
                    name: g.gate.label().to_string(),
                    passed: g.passed,
                    duration_ms: g.duration.as_millis() as u64,
                    command: g.command.clone(),
                })
                .collect(),
            total_duration_ms: result.total_duration().as_millis() as u64,
        }
    }
}

#[cfg(test)]
#[path = "envelope.tests.rs"]
mod tests;
