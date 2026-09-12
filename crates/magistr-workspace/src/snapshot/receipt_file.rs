use serde::{Deserialize, Serialize};

/// One file of a mutation, by hashes only; the journal keeps the bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReceiptFile {
    pub path: String,
    /// `create`, `write` or `delete`.
    pub kind: String,
    pub before_hash: String,
    pub after_hash: String,
}
