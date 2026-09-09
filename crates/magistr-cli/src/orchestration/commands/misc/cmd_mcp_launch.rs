use std::path::{Path, PathBuf};
use std::process::Stdio;

use color_eyre::eyre::{self, WrapErr};

use crate::mcp_target::McpTarget;
use crate::scaffold::WorkbenchCommand;

/// Launch a built-in endpoint without storing machine paths in host config.
pub async fn cmd_mcp_launch(target: McpTarget, dir: &Path) -> eyre::Result<()> {
    let root = project_root(dir)?;
    if target == McpTarget::Magistr {
        return super::cmd_mcp_serve(&root).await;
    }

    let resolved = WorkbenchCommand::resolve();
    if let Some(hint) = resolved.install_hint() {
        eyre::bail!("{hint}");
    }
    let context = || {
        format!(
            "could not launch Workbench executable {:?}; check WORKBENCH_MCP, \
         WORKBENCH_MCP_REPO and PATH (a checkout also requires Bun)",
            resolved.command
        )
    };

    let mut command = resolved.process_command(&root).wrap_err_with(context)?;
    command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    // Replace this process: the MCP client's PID, stdio, signals, EOF and
    // exit status now belong directly to Workbench. Never filter MCP output.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Err(command.exec()).wrap_err_with(context)
    }
    #[cfg(not(unix))]
    {
        let mut child = tokio::process::Command::from(command)
            .kill_on_drop(true)
            .spawn()
            .wrap_err_with(context)?;
        let status = tokio::select! {
            status = child.wait() => status.wrap_err("waiting for Workbench")?,
            signal = tokio::signal::ctrl_c() => {
                signal.wrap_err("waiting for shutdown")?;
                child.kill().await.wrap_err("stopping Workbench")?;
                std::process::exit(130);
            }
        };
        std::process::exit(status.code().unwrap_or(1));
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
