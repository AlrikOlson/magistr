//! Watch the agent process, or wait forever when there is none.

use magistr_process::ReapingChild as Child;

/// Exit code of a reaped child (`-1` when killed by a signal or unknown).
pub(crate) fn exit_code(status: std::io::Result<std::process::ExitStatus>) -> i32 {
    status.map(|s| s.code().unwrap_or(-1)).unwrap_or(-1)
}

/// Resolve with the exit code when the child exits. With no child (an
/// in-process agent under test) this never resolves, so it can sit in a
/// `select!` unconditionally.
pub(crate) async fn wait_child(child: &mut Option<Child>) -> i32 {
    match child {
        Some(child) => exit_code(child.wait().await),
        None => std::future::pending().await,
    }
}
