//! Recorded report outcomes; file reading and parsing stay with the workflow host.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// Stored on the check so the record says explicitly whether parsing
// happened, per the degradation contract in the module doc. Only the
// caller-facing lines are `///` — they ride the outputSchema wire.
/// What happened with the requested report.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct ReportRecord {
    /// The format key as requested.
    pub format: String,
    /// The report file path as requested.
    pub path: String,
    /// True when the file was read and parsed.
    pub parsed: bool,
    /// Why parsing did not happen, when it did not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// Additive and separately fallible: it never decides `passed` or `verified`.
/// The summary parsed from a machine-readable report.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct TestResults {
    pub total: u64,
    pub passed: u64,
    pub failed: u64,
    pub skipped: u64,
    /// Runner-reported duration, when reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    // Bounded to [`MAX_FAILURES`] entries; messages are redacted and
    // truncated to [`FAILURE_MESSAGE_CHARS`] before storage.
    /// Failing tests (bounded; messages redacted + truncated).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<TestFailure>,
}

// The addressable thing a red gate can finally point at.
/// One failing test.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct TestFailure {
    /// `classname::name` when the report carries both, else the bare name.
    pub name: String,
    /// The failure message; absent when the report carried none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[cfg(test)]
#[path = "report.tests.rs"]
mod tests;
