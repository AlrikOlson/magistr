use serde::Serialize;

use super::DoctorSection;

/// The whole report: sections in contract order and the exit-1 signal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorReport {
    pub workspace_root: String,
    pub generated_at: String,
    pub tool_version: String,
    pub sections: Vec<DoctorSection>,
    /// True when any finding is `unhealthy`.
    pub findings_present: bool,
}
