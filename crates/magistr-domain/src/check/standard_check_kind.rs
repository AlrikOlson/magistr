use core::fmt;
use serde::{Deserialize, Serialize};

/// Which check is being referenced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StandardCheckKind {
    /// Compilation/type check (e.g., `cargo check`, `tsc --noEmit`).
    Check,
    /// Test suite (e.g., `cargo nextest run`, `npm test`).
    Test,
    /// Linter (e.g., `cargo clippy`, `eslint`).
    Lint,
}

impl StandardCheckKind {
    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Check => "Check",
            Self::Test => "Test",
            Self::Lint => "Lint",
        }
    }
}

impl fmt::Display for StandardCheckKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}
