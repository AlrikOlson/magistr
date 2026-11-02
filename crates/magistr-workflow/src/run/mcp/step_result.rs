//! Compact mutation check_result; full step history stays behind the context query.
use crate::run::domain::step::Step;
use serde::Serialize;
#[derive(Serialize)]
pub(super) struct StepResult {
    #[serde(flatten)]
    summary: crate::run::output_schemas::StepOutput,
}
impl From<Step> for StepResult {
    fn from(step: Step) -> Self {
        Self {
            summary: crate::run::output_schemas::StepOutput {
                id: step.id,
                title: step.title,
                step_type: serde_json::to_value(step.step_type)
                    .unwrap_or_default()
                    .as_str()
                    .unwrap_or_default()
                    .into(),
                status: serde_json::to_value(step.status)
                    .unwrap_or_default()
                    .as_str()
                    .unwrap_or_default()
                    .into(),
                started_at: step.started_at,
                completed_at: step.completed_at,
                progress_count: step.progress.len(),
                checks_count: step.checks.len(),
                artifacts_count: step.artifacts.len(),
            },
        }
    }
}
#[cfg(test)]
#[path = "step_result.tests.rs"]
mod tests;
