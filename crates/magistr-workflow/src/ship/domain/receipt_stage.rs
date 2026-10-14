//! Durable progress of a check outcome through application to its task.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptStage {
    Admitted,
    Recorded,
    Applying,
    Settled,
}
