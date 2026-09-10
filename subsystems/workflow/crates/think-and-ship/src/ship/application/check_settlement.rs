use serde::{Deserialize, Serialize};

use crate::ship::domain::check::Check;

use super::{CheckAdmission, CheckOutcome};

/// A late result is returned intact to its caller, never reassigned to another
/// run or task. Durable receipt recovery is intentionally not implemented here.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "settlement", rename_all = "snake_case")]
pub enum CheckSettlement {
    Applied {
        check: Check,
    },
    Stale {
        admission: CheckAdmission,
        outcome: CheckOutcome,
    },
}
