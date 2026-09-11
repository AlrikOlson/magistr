use serde::{Deserialize, Serialize};

/// Execution lane selected by the trusted registration decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerLanguageLane {
    Native,
    Wasm,
}
