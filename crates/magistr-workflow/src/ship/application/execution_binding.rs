use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The admitted working copy and workflow owner serving one execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ExecutionBinding {
    pub working_copy: PathBuf,
    pub execution_id: String,
    pub owner_generation: u64,
}
