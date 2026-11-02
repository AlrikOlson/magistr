use super::{CheckAdmission, CheckOutcome};
use crate::run::domain::{check::Check, check_result::CheckResult};
use magistr_workflow_contracts::Observed;
use serde::{Deserialize, Serialize};

/// Each persisted record carries the provenance of its own commit.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "settlement", rename_all = "snake_case")]
pub enum CheckSettlement {
    Applied {
        check: Observed<Check>,
        check_result: Box<Observed<CheckResult>>,
    },
    Stale {
        admission: CheckAdmission,
        outcome: CheckOutcome,
        check_result: Box<Observed<CheckResult>>,
    },
}

impl CheckSettlement {
    #[must_use]
    pub fn check_result(&self) -> &Observed<CheckResult> {
        match self {
            Self::Applied { check_result, .. } | Self::Stale { check_result, .. } => check_result,
        }
    }
}
