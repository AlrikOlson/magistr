use super::domain::check_receipt::CheckReceipt;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Result of explicitly pruning older settled check receipts.
/// Retained counts are disjoint: current run, older unsettled checks, then
/// older settled checks referenced by unresolved host operations.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReceiptCompaction {
    pub removed: usize,
    pub retained_current: usize,
    pub retained_unsettled: usize,
    pub retained_referenced: usize,
}

impl ReceiptCompaction {
    pub(crate) fn retain(
        &mut self,
        receipt: &CheckReceipt,
        current_run: &str,
        referenced: &std::collections::BTreeSet<String>,
    ) -> bool {
        if receipt.target.run_id == current_run {
            self.retained_current += 1;
            true
        } else if receipt.settlement.is_none()
            || receipt.execution.is_none()
            || receipt.settled_at.is_none()
        {
            self.retained_unsettled += 1;
            true
        } else if referenced.contains(&receipt.id) {
            self.retained_referenced += 1;
            true
        } else {
            false
        }
    }
}
