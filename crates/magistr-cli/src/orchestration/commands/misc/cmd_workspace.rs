//! `magistr workspace <action>`: the native workspace tools, served.

use color_eyre::eyre;

use crate::cli::WorkspaceAction;

/// Dispatch one `magistr workspace` action.
pub async fn cmd_workspace(action: WorkspaceAction) -> eyre::Result<()> {
    match action {
        WorkspaceAction::Serve { dir, config } => {
            crate::workspace_mcp::run_workspace_stdio_server(&dir, &config).await
        }
    }
}
