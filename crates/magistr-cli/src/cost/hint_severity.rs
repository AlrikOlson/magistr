//! Severity levels for optimization hints.

/// Severity levels for optimization hints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HintSeverity {
    /// Informational — minor optimization opportunity.
    Info,
    /// Warning — noticeable waste, worth addressing.
    Warning,
    /// Critical — significant money being wasted.
    Critical,
}

impl std::fmt::Display for HintSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Warning => write!(f, "warning"),
            Self::Critical => write!(f, "critical"),
        }
    }
}
