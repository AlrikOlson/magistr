//! Validation warning for missing tools.

use std::fmt;

/// A validation warning about a potentially missing tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationWarning {
    pub tool: String,
    pub install_hint: String,
}

impl fmt::Display for ValidationWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} — install: {}", self.tool, self.install_hint)
    }
}
