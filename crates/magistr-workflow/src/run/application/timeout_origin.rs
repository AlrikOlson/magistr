use serde::{Deserialize, Serialize};

/// The configuration layer that selected a check's deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimeoutOrigin {
    PerCall,
    Gate,
    Project,
    Default,
}
