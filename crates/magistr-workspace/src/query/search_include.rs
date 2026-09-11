use serde::{Deserialize, Serialize};

/// Explicit exclusions; absence retains demoted test and vendor results.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SearchInclude {
    pub tests: Option<bool>,
    pub vendor: Option<bool>,
}
