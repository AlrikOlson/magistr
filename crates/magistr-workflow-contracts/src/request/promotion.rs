use crate::{Observed, Request, Task};

/// Each save carries its own committed source; the two owners are not one transaction.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RequestPromotion {
    pub request_id: String,
    pub task_id: String,
    pub created: bool,
    pub request: Observed<Request>,
    pub task: Observed<Task>,
}
