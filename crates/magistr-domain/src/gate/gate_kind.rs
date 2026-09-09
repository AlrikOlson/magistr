use core::fmt;
use serde::{Deserialize, Serialize};

/// Which quality gate is being referenced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateKind {
    /// Compilation/type check (e.g., `cargo check`, `tsc --noEmit`).
    Check,
    /// Test suite (e.g., `cargo nextest run`, `npm test`).
    Test,
    /// Linter (e.g., `cargo clippy`, `eslint`).
    Lint,
}

impl GateKind {
    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Check => "Check",
            Self::Test => "Test",
            Self::Lint => "Lint",
        }
    }
}

impl fmt::Display for GateKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}
