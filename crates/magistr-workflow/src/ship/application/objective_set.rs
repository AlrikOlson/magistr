use serde::Serialize;

use crate::ship::domain::objective::Objective;

/// Objective selection and the number of completed tasks retained in history.
#[derive(Debug, Clone, Serialize)]
pub struct ObjectiveSet {
    #[serde(flatten)]
    pub objective: Objective,
    #[serde(skip_serializing_if = "is_zero")]
    pub retained_tasks: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

impl ObjectiveSet {
    pub(crate) fn new(objective: Objective, retained_tasks: usize) -> Self {
        let note = (retained_tasks > 0).then(|| format!(
            "Started new work. The previous objective and its {retained_tasks} tasks remain in work history."
        ));
        Self {
            objective,
            retained_tasks,
            note,
        }
    }
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}
