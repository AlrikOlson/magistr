use serde::Serialize;

use crate::ship::domain::objective::Objective;

/// Objective update, including the established fresh-cycle notification.
#[derive(Debug, Clone, Serialize)]
pub struct ObjectiveSet {
    #[serde(flatten)]
    pub objective: Objective,
    #[serde(skip_serializing_if = "is_zero")]
    pub cleared_stale_tasks: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

impl ObjectiveSet {
    pub(crate) fn new(objective: Objective, cleared_stale_tasks: usize) -> Self {
        let note = (cleared_stale_tasks > 0).then(|| format!(
            "Started a fresh cycle: cleared {cleared_stale_tasks} task(s) left over from the previous (finished) objective."
        ));
        Self {
            objective,
            cleared_stale_tasks,
            note,
        }
    }
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}
