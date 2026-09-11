use crate::transaction::TransactionChange;

/// One verb's resolution: the exact changes to prepare, how they were derived,
/// and whether the verb is inherently confirm-gated.
pub(super) struct ResolvedWrites {
    pub changes: Vec<TransactionChange>,
    pub notes: Vec<String>,
    pub always_gated: bool,
}
