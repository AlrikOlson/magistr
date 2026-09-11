use super::{FileResult, TransactionError, TransactionStatus};

/// Truthful application receipt. A retained intent is not a source snapshot or
/// proof that a transaction completed, especially after process interruption.
#[derive(Debug)]
pub struct TransactionOutcome {
    pub status: TransactionStatus,
    pub files: Vec<FileResult>,
    pub journal_intent_seq: Option<i64>,
    pub error: Option<TransactionError>,
    pub notes: Vec<String>,
}
