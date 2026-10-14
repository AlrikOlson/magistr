//! Whether check receipts are held in memory or durably stored.
use serde::{Deserialize, Serialize};

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptStorage {
    #[default]
    Memory,
    File,
}
