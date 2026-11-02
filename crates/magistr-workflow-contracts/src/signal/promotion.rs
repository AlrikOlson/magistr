use crate::{Observed, Signal, Task};

/// Each save carries its own committed source; the two owners are not one transaction.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SignalPromotion {
    pub signal_id: String,
    pub task_id: String,
    pub created: bool,
    pub signal: Observed<Signal>,
    pub task: Observed<Task>,
}
