//! A suggestion for improving CLAUDE.md.

/// A suggestion for improving CLAUDE.md.
#[derive(Debug, Clone)]
pub struct Suggestion {
    /// Category of the suggestion (e.g., "error-handling", "testing").
    pub category: &'static str,
    /// The suggested rule or convention text.
    pub rule: String,
    /// Why this is recommended.
    pub rationale: String,
}

impl std::fmt::Display for Suggestion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {}\n  why: {}",
            self.category, self.rule, self.rationale
        )
    }
}
