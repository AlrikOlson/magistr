//! The identities one admitted check is bound to, kept apart.

use std::path::PathBuf;

use magistr_engine::native::{SourceObservation, StepAdmission};
use magistr_workspace::snapshot::WorkspaceIdentity;

/// Which workflow execution and which workspace an admitted check names.
///
/// The workflow side is the native `StepAdmission`: the project, the
/// goal's run, the step and the step instance within that run — a
/// later goal reusing the step name is a different instance. The
/// workspace side is the canonical root and the durable owner generation
/// that was current at capture, present only when the writer carries the
/// evidence capability. They are different authorities: a workflow
/// revision is comparable only within one project store, a workspace
/// generation advances on owner admission and never on a write, and the
/// source revision is content-addressed. No field here combines them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckIdentities {
    pub project_id: String,
    pub run_id: String,
    pub step_id: String,
    pub step_instance: String,
    pub workspace: Option<WorkspaceIdentity>,
}

impl CheckIdentities {
    pub(super) fn new(target: &StepAdmission, source: Option<&SourceObservation>) -> Self {
        Self {
            project_id: target.project_id.clone(),
            run_id: target.run_id.clone(),
            step_id: target.step_id.clone(),
            step_instance: target.step_instance.clone(),
            workspace: source.map(|observed| WorkspaceIdentity {
                root: PathBuf::from(&observed.workspace_root),
                generation: observed.workspace_generation,
            }),
        }
    }
}
