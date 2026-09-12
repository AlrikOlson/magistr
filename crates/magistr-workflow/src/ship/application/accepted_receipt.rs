use serde::{Deserialize, Serialize};

/// One receipt a completion accepted as current evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptedReceipt {
    pub gate: String,
    pub receipt_id: String,
    pub attempt: u64,
    pub source_revision: Option<String>,
}
