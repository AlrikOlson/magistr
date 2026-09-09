//! Output format selection for headless/CI mode.

use std::fmt;
use std::str::FromStr;

use clap::ValueEnum;

/// Output format for headless mode.
///
/// Controls how session events and gate results are rendered when running
/// without the TUI (e.g., in CI pipelines).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// NDJSON event stream (one JSON object per line to stdout)
    Json,
    /// Human-readable plain text summary (no ANSI colors)
    #[default]
    Text,
    /// GitHub Actions workflow annotations (::error::, ::warning::, ::notice::)
    #[value(alias = "gha")]
    GithubActions,
}

impl OutputFormat {
    /// Whether this format produces machine-readable output.
    #[must_use]
    pub fn is_machine_readable(self) -> bool {
        matches!(self, Self::Json)
    }
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json => write!(f, "json"),
            Self::Text => write!(f, "text"),
            Self::GithubActions => write!(f, "github-actions"),
        }
    }
}

impl FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "text" => Ok(Self::Text),
            "github-actions" | "gha" => Ok(Self::GithubActions),
            other => Err(format!(
                "unknown output format {other:?}, expected: json, text, github-actions"
            )),
        }
    }
}

#[cfg(test)]
#[path = "format.tests.rs"]
mod tests;
