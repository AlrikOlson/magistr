use serde::Serialize;

use super::{Diagnostic, RunFailure};

/// One run's answer. `report`: a runner parser structured the output.
/// `diagnostics`: a build, check or lint (or a test run that never
/// compiled), errors first. `output`: the honest head+tail fallback, cut on
/// the envelope's truncation manifest — never a silent guess at structure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum RunRow {
    #[serde(rename_all = "camelCase")]
    Report {
        command: String,
        resolved_line: String,
        adapter: String,
        exit_code: Option<i32>,
        timed_out: bool,
        duration_ms: u64,
        failures: Vec<RunFailure>,
        #[serde(skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Output {
        command: String,
        resolved_line: String,
        exit_code: Option<i32>,
        timed_out: bool,
        duration_ms: u64,
        stdout_head: Vec<String>,
        stdout_tail: Vec<String>,
        stderr_head: Vec<String>,
        stderr_tail: Vec<String>,
    },
    #[serde(rename_all = "camelCase")]
    Diagnostics {
        command: String,
        resolved_line: String,
        extractor: String,
        exit_code: Option<i32>,
        timed_out: bool,
        duration_ms: u64,
        diagnostics: Vec<Diagnostic>,
        /// Counts over the WHOLE run, before any budget cut.
        summary: String,
    },
}

impl RunRow {
    #[must_use]
    pub fn command(&self) -> &str {
        match self {
            Self::Report { command, .. }
            | Self::Output { command, .. }
            | Self::Diagnostics { command, .. } => command,
        }
    }

    #[must_use]
    pub fn exit_code(&self) -> Option<i32> {
        match self {
            Self::Report { exit_code, .. }
            | Self::Output { exit_code, .. }
            | Self::Diagnostics { exit_code, .. } => *exit_code,
        }
    }

    #[must_use]
    pub fn timed_out(&self) -> bool {
        match self {
            Self::Report { timed_out, .. }
            | Self::Output { timed_out, .. }
            | Self::Diagnostics { timed_out, .. } => *timed_out,
        }
    }
}
