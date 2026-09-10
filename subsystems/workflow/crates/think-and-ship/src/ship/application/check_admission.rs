use serde::{Deserialize, Serialize};

use super::{CheckRequest, TaskAdmission};

/// An admitted check retains its exact task and request across asynchronous work.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckAdmission {
    pub target: TaskAdmission,
    pub request: CheckRequest,
}
