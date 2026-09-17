use std::path::{Path, PathBuf};

use color_eyre::eyre::{self, WrapErr};

use crate::mcp_target::McpTarget;

/// Launch a built-in endpoint without storing machine paths in host config.
///
/// Both targets are this binary: `magistr` serves the workflow families,
/// `workbench-mcp` is the retained name of the native workspace bridge —
/// the same admitted owner every other Magistr surface reaches, never an
/// external executable, a checkout or a runtime looked up on this machine.
/// The launcher runs before the session's own capture, so it captures the
/// project inputs itself, once, for the endpoint it serves.
pub async fn cmd_mcp_launch(target: McpTarget, dir: &Path) -> eyre::Result<()> {
    let root = project_root(dir)?;
    match target {
        McpTarget::Magistr => super::cmd_mcp_serve(&root).await,
        McpTarget::Workbench => {
            let inputs = crate::config::inputs::ProjectInputs::from_env(&root);
            crate::workspace_mcp::run_workspace_stdio_server(&inputs, &[]).await
        }
    }
}

/// Walk only within the containing repository; a nested project wins. Without
/// a project marker, retain the supplied directory rather than guessing.
fn project_root(dir: &Path) -> eyre::Result<PathBuf> {
    let start = dir
        .canonicalize()
        .wrap_err_with(|| format!("invalid MCP workspace {}", dir.display()))?;
    if !start.is_dir() {
        eyre::bail!("MCP workspace is not a directory: {}", start.display());
    }
    for candidate in start.ancestors() {
        if candidate.join(".magistr/project.json").is_file()
            || candidate.join(".jig/project.json").is_file()
        {
            return Ok(candidate.to_path_buf());
        }
        if candidate.join(".git").exists() {
            break;
        }
    }
    Ok(start)
}
