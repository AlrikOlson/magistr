use serde::Serialize;

use super::DiagnosticLevel;

/// One compiler or lint diagnostic: a tool-layer row, never a runner
/// report — the public `workbench:runner@0.1.0` record stays frozen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    /// `E0308` | `unused_variables` | `clippy::needless_return`, when named.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// The diagnostic's own message, verbatim.
    pub message: String,
    /// Workspace-relative, forward slashes (the primary span).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u64>,
    /// The rendered snippet plus help/note lines, capped.
    pub detail: String,
}
