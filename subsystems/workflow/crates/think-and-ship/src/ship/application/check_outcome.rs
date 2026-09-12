use serde::{Deserialize, Serialize};

use crate::ship::report::{ReportRecord, TestResults};

use super::CheckExecution;

/// Execution data supplied to settlement: how the command ended, what it
/// said, and any report the runner was asked for.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
