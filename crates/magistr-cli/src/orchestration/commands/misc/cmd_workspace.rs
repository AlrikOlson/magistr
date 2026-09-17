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
        WorkspaceAction::Serve { dir, config } => {
            crate::workspace_mcp::run_workspace_stdio_server(&inputs.for_project(dir), &config)
                .await
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
        WorkspaceAction::Migrate { dir } => workspace_cli::run_migrate(&dir),
        WorkspaceAction::Snapshot { dir, artifact } => workspace_cli::run_snapshot(&dir, &artifact),
    }
}
