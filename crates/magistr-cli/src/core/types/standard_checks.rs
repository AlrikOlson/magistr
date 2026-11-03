use serde::{Deserialize, Serialize};

/// Check commands.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StandardChecks {
    pub check: String,
    pub test: String,
    pub lint: String,
}

/// A custom check defined in `project.json`.
///
/// Custom checks run alongside the standard check/test/lint checks when
/// `magistr_check(check: "all")` is invoked, and can also be run individually
/// by name.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustomCheck {
    /// Short identifier used as the check name (e.g. "structure", "security").
    pub name: String,
    /// Shell command to execute.
    pub command: String,
    /// Whether failure of this check should fail the overall "all" run.
    /// Defaults to `true`.
    #[serde(default = "default_true")]
    pub required: bool,
}

fn default_true() -> bool {
    true
}

/// Format configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FormatConfig {
    pub extensions: Vec<String>,
    pub command: String,
    /// Directories to exclude when formatting without explicit file list.
    ///
    /// Prevents formatters from scanning third-party code (e.g. `node_modules`,
    /// `target`, `.venv`). Populated with sensible defaults per language preset.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclusions: Vec<String>,
}

/// Testing configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TestingConfig {
    pub companion_pattern: String,
    pub test_dir: String,
}
