use serde::Serialize;

use crate::ship::domain::{
    action::Action,
    artifact::ArtifactType,
    check::CheckType,
    objective::Objective,
    task::{Task, TaskStatus, TaskType},
};

use super::ShipSnapshot;

/// Typed current status, retaining the established transport response fields.
#[derive(Debug, Clone, Serialize)]
pub struct ShipStatus {
    project_id: String,
    objective: Option<Objective>,
    tasks: StatusCounts,
    task_list: Vec<TaskSummary>,
    active_task: Option<Task>,
    recent_actions: Vec<Action>,
    checks: Vec<CheckRef>,
    artifacts: Vec<ArtifactRef>,
    think_refs: Vec<ThinkRef>,
}

impl ShipStatus {
    pub(crate) fn from_snapshot(snapshot: &ShipSnapshot) -> Self {
        let count = |status| {
            snapshot
                .tasks
                .iter()
                .filter(|task| task.status == status)
                .count()
        };
        Self {
            project_id: snapshot.project_id.clone(),
            objective: snapshot.objective.clone(),
            tasks: StatusCounts {
                planned: count(TaskStatus::Planned),
                active: count(TaskStatus::Active),
                blocked: count(TaskStatus::Blocked),
                completed: count(TaskStatus::Completed),
                skipped: count(TaskStatus::Skipped),
                total: snapshot.tasks.len(),
            },
            task_list: snapshot.tasks.iter().map(TaskSummary::from).collect(),
            active_task: snapshot.active_task().cloned(),
            recent_actions: snapshot
                .tasks
                .iter()
                .flat_map(|task| &task.actions)
                .rev()
                .take(5)
                .cloned()
                .collect(),
            checks: snapshot
                .tasks
                .iter()
                .flat_map(|task| {
                    task.checks.iter().map(|check| CheckRef {
                        task_id: task.id.clone(),
                        name: check.name.clone(),
                        check_type: check.check_type.clone(),
                        passed: check.passed,
                        required: check.required,
                        verified: check.verified,
                    })
                })
                .collect(),
            artifacts: snapshot
                .tasks
                .iter()
                .flat_map(|task| {
                    task.artifacts.iter().map(|artifact| ArtifactRef {
                        task_id: task.id.clone(),
                        artifact_type: artifact.artifact_type.clone(),
                        reference: artifact.reference.clone(),
                        description: artifact.description.clone(),
                    })
                })
                .collect(),
            think_refs: snapshot.tasks.iter().flat_map(think_refs).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct StatusCounts {
    planned: usize,
    active: usize,
    blocked: usize,
    completed: usize,
    skipped: usize,
    total: usize,
}

#[derive(Debug, Clone, Serialize)]
struct TaskSummary {
    id: String,
    title: String,
    #[serde(rename = "type")]
    task_type: TaskType,
    status: TaskStatus,
    estimate: Option<String>,
    actions_count: usize,
    checks_count: usize,
    artifacts_count: usize,
}

impl From<&Task> for TaskSummary {
    fn from(task: &Task) -> Self {
        Self {
            id: task.id.clone(),
            title: task.title.clone(),
            task_type: task.task_type.clone(),
            status: task.status,
            estimate: task.estimate.clone(),
            actions_count: task.actions.len(),
            checks_count: task.checks.len(),
            artifacts_count: task.artifacts.len(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct CheckRef {
    task_id: String,
    name: String,
    #[serde(rename = "type")]
    check_type: CheckType,
    passed: bool,
    required: bool,
    verified: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ArtifactRef {
    task_id: String,
    #[serde(rename = "type")]
    artifact_type: ArtifactType,
    #[serde(rename = "ref")]
    reference: String,
    description: String,
}

#[derive(Debug, Clone, Serialize)]
struct ThinkRef {
    task_id: String,
    ref_type: &'static str,
    value: ThinkValue,
    #[serde(skip_serializing_if = "Option::is_none")]
    action_id: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
enum ThinkValue {
    Branch(String),
    Step(u32),
}

fn think_refs(task: &Task) -> Vec<ThinkRef> {
    let mut refs = Vec::new();
    if let Some(branch) = &task.think_branch {
        refs.push(ThinkRef {
            task_id: task.id.clone(),
            ref_type: "branch",
            value: ThinkValue::Branch(branch.clone()),
            action_id: None,
        });
    }
    for action in &task.actions {
        if let Some(step) = action.think_step {
            refs.push(ThinkRef {
                task_id: task.id.clone(),
                ref_type: "step",
                value: ThinkValue::Step(step),
                action_id: Some(action.id),
            });
        }
    }
    refs
}
