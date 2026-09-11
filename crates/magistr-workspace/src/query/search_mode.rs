use serde::{Deserialize, Serialize};

/// Native search lanes; semantic remains an explicitly unsupported mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchMode {
    #[default]
    Auto,
    Lexical,
    Structural,
    References,
    Files,
    Semantic,
}
