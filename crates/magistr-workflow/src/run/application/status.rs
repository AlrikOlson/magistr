use serde::Serialize;

use crate::run::domain::{
    artifact::ArtifactType,
    check::CheckType,
    goal::Goal,
    progress::Progress,
    step::{Step, StepStatus, StepType},
};

use super::RunSnapshot;

/// Typed current status, retaining the established transport response fields.
#[derive(Debug, Clone, Serialize)]
pub struct RunStatus {
    project_id: String,
    execution: Option<super::ExecutionBinding>,
    run_id: String,
    goal: Option<Goal>,
    steps: StatusCounts,
    step_list: Vec<StepSummary>,
    active_step: Option<Step>,
    recent_progress: Vec<Progress>,
    checks: Vec<CheckRef>,
    artifacts: Vec<ArtifactRef>,
    decision_refs: Vec<DecisionRef>,
}

impl RunStatus {
    pub(crate) fn from_snapshot(snapshot: &RunSnapshot) -> Self {
        let count = |status| {
            snapshot
                .steps
                .iter()
                .filter(|step| step.status == status)
                .count()
        };
        Self {
            project_id: snapshot.project_id.clone(),
            execution: snapshot.execution.clone(),
            run_id: snapshot.run_id.clone(),
            goal: snapshot.goal.clone(),
            steps: StatusCounts {
                planned: count(StepStatus::Planned),
                active: count(StepStatus::Active),
                waiting: count(StepStatus::Waiting),
                done: count(StepStatus::Done),
                skipped: count(StepStatus::Skipped),
                total: snapshot.steps.len(),
            },
            step_list: snapshot.steps.iter().map(StepSummary::from).collect(),
            active_step: snapshot.active_step().cloned(),
            recent_progress: snapshot
                .steps
                .iter()
                .flat_map(|step| &step.progress)
                .rev()
                .take(5)
                .cloned()
                .collect(),
            checks: snapshot
                .steps
                .iter()
                .flat_map(|step| {
                    step.checks.iter().map(|check| CheckRef {
                        step_id: step.id.clone(),
                        name: check.name.clone(),
                        check_type: check.check_type.clone(),
                        passed: check.passed,
                        required: check.required,
                        verified: check.verified,
                    })
                })
                .collect(),
            artifacts: snapshot
                .steps
                .iter()
                .flat_map(|step| {
                    step.artifacts.iter().map(|artifact| ArtifactRef {
                        step_id: step.id.clone(),
                        artifact_type: artifact.artifact_type.clone(),
                        reference: artifact.reference.clone(),
                        description: artifact.description.clone(),
                    })
                })
                .collect(),
            decision_refs: snapshot.steps.iter().flat_map(decision_refs).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct StatusCounts {
    planned: usize,
    active: usize,
    waiting: usize,
    done: usize,
    skipped: usize,
    total: usize,
}

#[derive(Debug, Clone, Serialize)]
struct StepSummary {
    id: String,
    title: String,
    #[serde(rename = "type")]
    step_type: StepType,
    status: StepStatus,
    estimate: Option<String>,
    progress_count: usize,
    checks_count: usize,
    artifacts_count: usize,
}

impl From<&Step> for StepSummary {
    fn from(step: &Step) -> Self {
        Self {
            id: step.id.clone(),
            title: step.title.clone(),
            step_type: step.step_type.clone(),
            status: step.status,
            estimate: step.estimate.clone(),
            progress_count: step.progress.len(),
            checks_count: step.checks.len(),
            artifacts_count: step.artifacts.len(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct CheckRef {
    step_id: String,
    name: String,
    #[serde(rename = "type")]
    check_type: CheckType,
    passed: bool,
    required: bool,
    verified: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ArtifactRef {
    step_id: String,
    #[serde(rename = "type")]
    artifact_type: ArtifactType,
    #[serde(rename = "ref")]
    reference: String,
    description: String,
}

#[derive(Debug, Clone, Serialize)]
struct DecisionRef {
    step_id: String,
    ref_type: &'static str,
    value: DecisionValue,
    #[serde(skip_serializing_if = "Option::is_none")]
    progress_id: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
enum DecisionValue {
    Branch(String),
    Step(u32),
}

fn decision_refs(step: &Step) -> Vec<DecisionRef> {
    let mut refs = Vec::new();
    if let Some(branch) = &step.decision_thread {
        refs.push(DecisionRef {
            step_id: step.id.clone(),
            ref_type: "branch",
            value: DecisionValue::Branch(branch.clone()),
            progress_id: None,
        });
    }
    for progress in &step.progress {
        if let Some(note) = progress.note {
            refs.push(DecisionRef {
                step_id: step.id.clone(),
                ref_type: "step",
                value: DecisionValue::Step(note),
                progress_id: Some(progress.id),
            });
        }
    }
    refs
}
