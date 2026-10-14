//! The answer to settling an admitted check.

use magistr_engine::native::{Check, CheckReceipt, CheckSettlement, SourceFreshness};

/// One settled check: the workflow's settlement — `Applied` with the
/// recorded check, or `Stale` with the admission and outcome intact when
/// the run or task was superseded while the gate ran — and, either way,
/// the durable receipt settled under the admitted identity, carrying the
/// execution, the timing and whether the source still matched.
#[derive(Clone, Debug)]
pub struct SettledCheck {
    pub settlement: CheckSettlement,
}

impl SettledCheck {
    /// The recorded check when the settlement applied.
    #[must_use]
    pub fn check(&self) -> Option<&Check> {
        match &self.settlement {
            CheckSettlement::Applied { check, .. } => Some(&check.value),
            CheckSettlement::Stale { .. } => None,
        }
    }

    /// The receipt, applied or stale.
    #[must_use]
    pub fn receipt(&self) -> &CheckReceipt {
        &self.settlement.receipt().value
    }

    /// Whether the workspace still held the admitted bytes at settlement.
    #[must_use]
    pub fn freshness(&self) -> Option<&SourceFreshness> {
        self.receipt().freshness.as_ref()
    }

    /// Whether the workflow accepted the result under its captured identity.
    #[must_use]
    pub const fn is_applied(&self) -> bool {
        matches!(self.settlement, CheckSettlement::Applied { .. })
    }
}
