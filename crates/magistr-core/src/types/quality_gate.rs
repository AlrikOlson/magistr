use serde::{Deserialize, Serialize};

/// Quality gate commands — the three standard gates that every project defines.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QualityGate {
    /// Type-check / compilation command (e.g., `"cargo check --all-targets"`).
    pub check: String,
    /// Test runner command (e.g., `"cargo nextest run"`, `"npm test"`).
    pub test: String,
    /// Linter command (e.g., `"cargo clippy -- -D warnings"`, `"eslint ."`).
    pub lint: String,
}

/// A custom quality gate defined in `project.json`.
///
/// Custom gates run alongside the standard check/test/lint gates when
/// `magistr_gate(gate: "all")` is invoked, and can also be run individually
/// by name.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustomGate {
    /// Short identifier used as the gate name (e.g. "structure", "security").
    pub name: String,
    /// Shell command to execute.
    pub command: String,
    /// Whether failure of this gate should fail the overall "all" run.
    /// Defaults to `true`.
    #[serde(default = "default_true")]
    pub required: bool,
}

fn default_true() -> bool {
    true
}

/// Code formatter configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FormatConfig {
    /// File extensions to format (e.g., `["rs"]`, `["ts", "tsx"]`).
    pub extensions: Vec<String>,
    /// Formatter command (e.g., `"cargo fmt"`, `"prettier --write ."`).
    pub command: String,
    /// Directories to exclude when formatting without explicit file list.
    ///
    /// Prevents formatters from scanning third-party code (e.g. `node_modules`,
    /// `target`, `.venv`). Populated with sensible defaults per language preset.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclusions: Vec<String>,
}

/// Testing configuration — where tests live and how they're named.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TestingConfig {
    /// File naming pattern for companion test files (e.g., `"{name}.tests.{ext}"`).
    pub companion_pattern: String,
    /// Test directory strategy: `"alongside"` (next to source) or `"tests"` (separate dir).
    pub test_dir: String,
}
