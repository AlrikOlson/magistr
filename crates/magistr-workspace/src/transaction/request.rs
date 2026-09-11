use super::{FormatMode, TransactionChange};

/// Typed mutation plus exact original request bytes and caller identity.
#[derive(Clone, Debug)]
pub struct TransactionRequest {
    pub operation: String,
    pub serialized_request: Vec<u8>,
    pub session_id: String,
    pub changes: Vec<TransactionChange>,
    pub undo_of: Vec<i64>,
    pub format: FormatMode,
}
