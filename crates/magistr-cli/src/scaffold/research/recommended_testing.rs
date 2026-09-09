use serde::Deserialize;

/// AI-recommended testing configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RecommendedTesting {
    /// Test companion file pattern (e.g. `"tests/{name}.rs"`).
    #[serde(default)]
    pub companion_pattern: Option<String>,
    /// Test directory (e.g. `"tests"`).
    #[serde(default)]
    pub test_dir: Option<String>,
}
