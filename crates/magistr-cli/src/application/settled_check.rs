//! The answer to settling an admitted check.

use magistr_engine::tas::{Check, CheckSettlement};

use super::CheckSource;

/// One settled check: the workflow's settlement — `Applied` with the
/// recorded check, or `Stale` with the admission and outcome intact when
/// the run or task was superseded while the gate ran — beside the source
/// evidence when the check was admitted with it. Nothing here is durable
/// beyond the ordinary check the workflow appended.
#[derive(Clone, Debug)]
pub struct SettledCheck {
    pub settlement: CheckSettlement,
    pub source: Option<CheckSource>,
}

impl SettledCheck {
    /// The recorded check when the settlement applied.
    #[must_use]
    pub fn check(&self) -> Option<&Check> {
        match &self.settlement {
            CheckSettlement::Applied { check } => Some(check),
            CheckSettlement::Stale { .. } => None,
        }
    }

    /// Whether the workflow accepted the result under its captured identity.
    #[must_use]
    pub const fn is_applied(&self) -> bool {
        matches!(self.settlement, CheckSettlement::Applied { .. })
    }
}
