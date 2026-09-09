use serde::Deserialize;

/// AI-recommended format configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RecommendedFormat {
    /// File extensions to format (e.g. `["rs"]`).
    #[serde(default)]
    pub extensions: Option<Vec<String>>,
    /// Format command (e.g. `"cargo fmt"`).
    #[serde(default)]
    pub command: Option<String>,
}
