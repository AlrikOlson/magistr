use serde::{Deserialize, Serialize};

use crate::ship::domain::check::Check;
use crate::ship::domain::check_receipt::CheckReceipt;

use super::{CheckAdmission, CheckOutcome};

/// A late result is returned intact to its caller, never reassigned to another
/// run or task; either way its receipt was settled under the admitted
/// identity and is returned with it.
// One settlement is answered per check and destructured at once by its
// caller; the variants differ in size because a stale result must carry
// its whole admission back, and boxing that would only move the same bytes.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "settlement", rename_all = "snake_case")]
pub enum CheckSettlement {
    Applied {
        check: Check,
        receipt: Box<CheckReceipt>,
    },
    Stale {
        admission: CheckAdmission,
        outcome: CheckOutcome,
        receipt: Box<CheckReceipt>,
    },
}

impl CheckSettlement {
    /// The receipt either way.
    #[must_use]
    pub fn receipt(&self) -> &CheckReceipt {
        match self {
            Self::Applied { receipt, .. } | Self::Stale { receipt, .. } => receipt,
        }
    }
}
