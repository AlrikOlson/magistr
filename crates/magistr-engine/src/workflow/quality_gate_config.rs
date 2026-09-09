use serde::{Deserialize, Serialize};

/// Quality gate command configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityGateConfig {
    pub check: String,
    pub test: String,
    pub lint: String,
}
