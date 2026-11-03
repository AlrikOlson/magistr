use serde::{Deserialize, Serialize};

/// Check command configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardChecksConfig {
    pub check: String,
    pub test: String,
    pub lint: String,
}
