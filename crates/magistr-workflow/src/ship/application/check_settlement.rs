use super::{CheckAdmission, CheckOutcome};
use crate::ship::domain::{check::Check, check_receipt::CheckReceipt};
use magistr_workflow_contracts::Observed;
use serde::{Deserialize, Serialize};

/// Each persisted record carries the provenance of its own commit.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "settlement", rename_all = "snake_case")]
pub enum CheckSettlement {
    Applied {
        check: Observed<Check>,
        receipt: Box<Observed<CheckReceipt>>,
    },
    Stale {
        admission: CheckAdmission,
        outcome: CheckOutcome,
        receipt: Box<Observed<CheckReceipt>>,
    },
}

impl CheckSettlement {
    #[must_use]
    pub fn receipt(&self) -> &Observed<CheckReceipt> {
        match self {
            Self::Applied { receipt, .. } | Self::Stale { receipt, .. } => receipt,
        }
    }
}
