//! Workspace admission for interactive and headless commands.

use crate::application::WorkspaceAdmission;

/// Admit the run's workspace — the captured project's root, on the captured
/// configuration layers — in this process (the composition every
/// source-writing host shares) and `announce` one line saying what the
/// session's source writes will do: run through the admitted pipeline, or
/// be refused because a live owner holds the workspace or this binary
/// cannot admit the root.
pub(crate) async fn admit_workspace(
    inputs: &crate::config::inputs::ProjectInputs,
    announce: impl FnOnce(String),
) -> WorkspaceAdmission {
    let admission = WorkspaceAdmission::admit(inputs).await;
    announce(admission.report());
    admission
}
