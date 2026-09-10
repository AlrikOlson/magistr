use serde::{Deserialize, Serialize};

use crate::ship::report::{ReportRecord, TestResults};

/// Execution data supplied to settlement; source-bound receipts are separate work.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckOutcome {
    pub passed: bool,
    pub details: String,
    pub verified: bool,
    pub command: Option<String>,
    pub exit_code: Option<i32>,
    pub report: Option<ReportRecord>,
    pub results: Option<TestResults>,
}
