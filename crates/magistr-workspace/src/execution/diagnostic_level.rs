use serde::Serialize;

/// Compiler and lint diagnostics keep only the two levels a run row reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticLevel {
    Error,
    Warning,
}
