use super::check_result_storage::CheckResultStorage;
use super::domain::check_result::CheckResult;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Result of explicitly pruning older settled check check_results.
/// Retained counts are disjoint: current run, older unsettled checks, then
/// older settled checks referenced by unresolved host operations.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CheckResultCompaction {
    pub storage: CheckResultStorage,
    pub removed: usize,
    pub retained_current: usize,
    pub retained_unsettled: usize,
    pub retained_referenced: usize,
}

impl CheckResultCompaction {
    pub(crate) fn retain(
        &mut self,
        check_result: &CheckResult,
        current_run: &str,
        referenced: &std::collections::BTreeSet<String>,
    ) -> bool {
        if check_result.target.run_id == current_run {
            self.retained_current += 1;
            true
        } else if check_result.settlement.is_none()
            || check_result.outcome.is_none()
            || check_result.settled_at.is_none()
        {
            self.retained_unsettled += 1;
            true
        } else if referenced.contains(&check_result.id) {
            self.retained_referenced += 1;
            true
        } else {
            false
        }
    }
}
