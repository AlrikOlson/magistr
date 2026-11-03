use serde::Deserialize;

/// AI-recommended check commands.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct RecommendedChecks {
    /// Build/check command (e.g. `"cargo check --all-targets"`).
    #[serde(default)]
    pub check: Option<String>,
    /// Test command (e.g. `"cargo test"`).
    #[serde(default)]
    pub test: Option<String>,
    /// Lint command (e.g. `"cargo clippy -- -D warnings"`).
    #[serde(default)]
    pub lint: Option<String>,
}
