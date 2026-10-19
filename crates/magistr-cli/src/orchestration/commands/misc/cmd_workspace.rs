//! `magistr workspace <action>`: the native workspace tools, served, and the
//! operational verbs beside them.

use color_eyre::eyre;

use crate::{
    cli::{WorkspaceAction, WorkspaceKeyAction},
    workspace_cli::{self, JournalView},
};

/// Dispatch one `magistr workspace` action.
pub async fn cmd_workspace(
    action: WorkspaceAction,
    inputs: &crate::config::inputs::ProjectInputs,
) -> eyre::Result<()> {
    match action {
        WorkspaceAction::WorkingCopy { dir, action } => {
            workspace_cli::run_working_copy(&dir, action)
        }
        WorkspaceAction::Doctor { mode, dir, config } => {
            workspace_cli::run_doctor(mode, &dir, &config, inputs)
        }
        WorkspaceAction::Journal {
            dir,
            limit,
            since,
            before,
            session,
            no_diff,
            json,
        } => workspace_cli::run_journal(
            &dir,
            &JournalView {
                limit,
                since,
                before,
                session,
                diff: !no_diff,
                json,
            },
        ),
        WorkspaceAction::Key {
            action: WorkspaceKeyAction::Rotate { dir },
        } => workspace_cli::run_key_rotate(&dir),
        WorkspaceAction::Init { dir } => workspace_cli::run_init(&dir),
        WorkspaceAction::Snapshot { dir, artifact } => workspace_cli::run_snapshot(&dir, &artifact),
    }
}
