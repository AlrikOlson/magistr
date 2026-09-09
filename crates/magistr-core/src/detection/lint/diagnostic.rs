//! A single lint diagnostic produced by a rule.

use super::severity::Severity;

/// A single lint diagnostic produced by a rule.
#[derive(Debug, Clone)]
pub struct LintDiagnostic {
    /// Which rule produced this diagnostic.
    pub rule: &'static str,
    /// Severity level.
    pub severity: Severity,
    /// Human-readable message describing the problem.
    pub message: String,
    /// Optional line number (1-based) where the issue occurs.
    pub line: Option<usize>,
    /// Actionable suggestion for fixing the issue.
    pub fix: Option<String>,
}

impl std::fmt::Display for LintDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let loc = self.line.map(|l| format!(":{l}")).unwrap_or_default();
        write!(f, "[{}] {}{}", self.severity, self.rule, loc)?;
        write!(f, ": {}", self.message)?;
        if let Some(fix) = &self.fix {
            write!(f, "\n  fix: {fix}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "diagnostic.tests.rs"]
mod tests;
