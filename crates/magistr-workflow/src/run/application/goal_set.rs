use serde::Serialize;

use crate::run::domain::goal::Goal;

/// Goal selection and the number of completed steps retained in history.
#[derive(Debug, Clone, Serialize)]
pub struct GoalSet {
    #[serde(flatten)]
    pub goal: Goal,
    #[serde(skip_serializing_if = "is_zero")]
    pub retained_steps: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

impl GoalSet {
    pub(crate) fn new(goal: Goal, retained_steps: usize) -> Self {
        let note = (retained_steps > 0).then(|| format!(
            "Started new work. The previous goal and its {retained_steps} steps remain in work history."
        ));
        Self {
            goal,
            retained_steps,
            note,
        }
    }
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}
