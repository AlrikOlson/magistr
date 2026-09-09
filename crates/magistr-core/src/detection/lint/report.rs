//! Result of running all lint rules on a CLAUDE.md file.

use super::diagnostic::LintDiagnostic;
use super::severity::Severity;

/// Result of running all lint rules on a CLAUDE.md file.
#[derive(Debug, Clone)]
pub struct LintReport {
    /// Path to the file that was linted.
    pub path: String,
    /// All diagnostics found.
    pub diagnostics: Vec<LintDiagnostic>,
}

impl LintReport {
    /// Count of errors.
    pub fn errors(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count()
    }

    /// Count of warnings.
    pub fn warnings(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .count()
    }

    /// Whether the lint passed (no errors).
    pub fn passed(&self) -> bool {
        self.errors() == 0
    }
}

#[cfg(test)]
#[path = "report.tests.rs"]
mod tests;
