//! Spawn the agent subprocess.

use std::env;
use std::path::Path;
use std::process::Stdio;

use magistr_process::ReapingChild;
use tokio::process::{ChildStderr, ChildStdin, ChildStdout, Command};

use crate::{AcpError, Result};

/// The spawned agent and its pipes, split out so the transport can wire each
/// one up separately.
pub(crate) struct SpawnedAgent {
    pub child: ReapingChild,
    pub group: Option<magistr_process::ProcessGroup>,
    pub stdin: ChildStdin,
    pub stdout: ChildStdout,
    pub stderr: ChildStderr,
}

/// Build the agent command: piped stdio, filtered environment, own process
/// group, killed on drop.
pub(crate) fn build_command(cli_path: &Path, cli_args: &[String], cwd: Option<&Path>) -> Command {
    let mut cmd = Command::new(cli_path);
    cmd.args(cli_args);
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    configure_agent_subprocess(&mut cmd);

    // The agent inherits our environment minus the loader/injection variables
    // and the nested-session guard, and learns who launched it.
    let mut env_vars: std::collections::HashMap<String, String> = env::vars().collect();
    env_vars.remove("CLAUDECODE");
    for var in [
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "DYLD_INSERT_LIBRARIES",
        "DYLD_LIBRARY_PATH",
        "NODE_OPTIONS",
    ] {
        env_vars.remove(var);
    }
    env_vars.insert(
        "CLAUDE_CODE_ENTRYPOINT".to_string(),
        "sdk-rust-acp".to_string(),
    );
    cmd.env_clear().envs(env_vars);
    cmd
}

/// Spawn `cmd` and take its three pipes.
pub(crate) fn spawn_agent(mut cmd: Command) -> Result<SpawnedAgent> {
    let program = cmd.as_std().get_program().to_string_lossy().into_owned();
    #[cfg(not(windows))]
    let mut child = ReapingChild::new(cmd.spawn().map_err(|e| {
        AcpError::connection(format!("Failed to spawn ACP agent `{program}`: {e}"))
    })?);
    #[cfg(windows)]
    let (mut child, group) = magistr_process::ProcessGroup::spawn(&mut cmd)
        .map_err(|e| AcpError::connection(format!("Failed to spawn ACP agent `{program}`: {e}")))?;
    // SAFETY: build_command configures an isolated process group before exec.
    #[cfg(not(windows))]
    let group = child
        .id()
        .map(|pid| unsafe { magistr_process::ProcessGroup::from_spawned_pid(pid) });
    #[cfg(windows)]
    let group = Some(group);
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| AcpError::connection("Failed to get ACP subprocess stdin"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AcpError::connection("Failed to get ACP subprocess stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AcpError::connection("Failed to get ACP subprocess stderr"))?;
    Ok(SpawnedAgent {
        child,
        group,
        stdin,
        stdout,
        stderr,
    })
}

/// Own process group (so `killpg` reaps the whole tree) and, on Linux, die
/// with the parent.
#[cfg(target_os = "linux")]
fn configure_agent_subprocess(cmd: &mut Command) {
    // SAFETY: `prctl` and `setpgid` are async-signal-safe and run between fork and exec.
    unsafe {
        cmd.pre_exec(|| {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
            if libc::setpgid(0, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
fn configure_agent_subprocess(cmd: &mut Command) {
    // SAFETY: `setpgid` is async-signal-safe and runs between fork and exec.
    unsafe {
        cmd.pre_exec(|| {
            if libc::setpgid(0, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[cfg(not(unix))]
fn configure_agent_subprocess(_cmd: &mut Command) {}
