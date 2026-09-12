use serde::{Deserialize, Serialize};

/// How a receipt's result was settled against the workflow: applied to the
/// task instance it was admitted on, or kept stale under its own identity
/// because that instance, run or project was no longer current.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "settlement", rename_all = "snake_case")]
pub enum ReceiptSettlement {
    Applied,
    Stale { reason: String },
}
