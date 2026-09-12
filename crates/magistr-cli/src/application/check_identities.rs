//! The identities one admitted check is bound to, kept apart.

use magistr_engine::tas::TaskAdmission;
use magistr_workspace::snapshot::WorkspaceIdentity;

/// Which workflow execution and which workspace an admitted check names.
///
/// The workflow side is the native `TaskAdmission`: the project, the
/// objective's run, the task and the task instance within that run — a
/// later objective reusing the task name is a different instance. The
/// workspace side is the canonical root and the durable owner generation
/// that was current at capture, present only when the consumer asked for
/// source evidence. They are different authorities: a workflow revision
/// is comparable only within one project store, a workspace generation
/// advances on owner admission and never on a write, and the source
/// revision is content-addressed. No field here combines them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckIdentities {
    pub project_id: String,
    pub run_id: String,
    pub task_id: String,
    pub task_instance: String,
    pub workspace: Option<WorkspaceIdentity>,
}

impl CheckIdentities {
    pub(super) fn new(target: &TaskAdmission, workspace: Option<WorkspaceIdentity>) -> Self {
        Self {
            project_id: target.project_id.clone(),
            run_id: target.run_id.clone(),
            task_id: target.task_id.clone(),
            task_instance: target.task_instance.clone(),
            workspace,
        }
    }
}
