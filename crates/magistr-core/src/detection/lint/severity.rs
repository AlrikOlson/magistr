//! Severity level for lint diagnostics.

/// Severity level for lint diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Informational suggestion — not a problem, but could be better.
    Info,
    /// Likely problem that may cause Claude to behave unexpectedly.
    Warning,
    /// Definite problem that will cause issues.
    Error,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

#[cfg(test)]
#[path = "severity.tests.rs"]
mod tests;
