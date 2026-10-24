use serde::{Deserialize, Serialize};

use crate::ship::report::{ReportRecord, TestResults};

use super::CheckExecution;

/// Execution data supplied to settlement: how the command ended, what it
/// said, and any report the runner was asked for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CheckOutcome {
    pub passed: bool,
    pub details: String,
    pub verified: bool,
    pub command: Option<String>,
    pub exit_code: Option<i32>,
    pub execution: CheckExecution,
    pub report: Option<ReportRecord>,
    pub results: Option<TestResults>,
}

impl CheckOutcome {
    pub(crate) fn reuse_unavailable(command: Option<String>) -> Self {
        let reason = "No matching reusable result was found; the command was not run.".to_owned();
        Self {
            passed: false,
            verified: false,
            details: reason.clone(),
            command,
            exit_code: None,
            execution: CheckExecution::ReuseUnavailable { reason },
            report: None,
            results: None,
        }
    }
}
