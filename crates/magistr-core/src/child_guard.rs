//! Global child process registry for orphan prevention.
//!
//! Tracks process group IDs (PGIDs) of child subprocesses spawned by magistr.
//! Signal handlers and exit hooks call [`kill_all_children`] to ensure no
//! orphans survive — even when `Drop` impls don't run on hard exit.
//!
//! Shared between the CLI binary and the Tauri GUI crate.
//!
//! ## Why a global?
//!
//! `std::process::exit()` terminates immediately without unwinding or running
//! destructors.  The signal handler (a tokio task, not a raw POSIX handler) needs
//! a static location to find the PGIDs it must kill.  A `Mutex<Vec<i32>>` is safe
//! here because the handler is an async task, not a signal-context function.

use std::sync::Mutex;

/// Active child process group IDs.
///
/// Entries are added by [`register_child_pgid`] after spawning a subprocess
/// in its own process group (`setpgid(0, 0)`).  Entries are removed by
/// [`unregister_child_pgid`] when the subprocess is confirmed dead.
static CHILD_PGIDS: Mutex<Vec<i32>> = Mutex::new(Vec::new());

/// Register a process group ID for cleanup on exit.
///
/// Called after spawning a subprocess whose `pre_exec` called `setpgid(0, 0)`.
/// PGID equals the child's PID in that case.
pub fn register_child_pgid(pgid: i32) {
    if pgid <= 0 {
        return;
    }
    if let Ok(mut pgids) = CHILD_PGIDS.lock()
        && !pgids.contains(&pgid)
    {
        pgids.push(pgid);
    }
}

/// Unregister a process group ID after confirmed cleanup.
pub fn unregister_child_pgid(pgid: i32) {
    if let Ok(mut pgids) = CHILD_PGIDS.lock() {
        pgids.retain(|&p| p != pgid);
    }
}

/// Gracefully kill every tracked child process group.
///
/// Sends `SIGTERM` first to allow graceful cleanup, waits briefly, then
/// sends `SIGKILL` to ensure termination.  Called by signal handlers and
/// exit hooks.
///
/// Killing an already-dead group returns `ESRCH` — harmless and ignored.
#[cfg(unix)]
pub fn kill_all_children() {
    if let Ok(pgids) = CHILD_PGIDS.lock() {
        for &pgid in pgids.iter() {
            if pgid > 0 {
                // SAFETY: killpg is async-signal-safe; ESRCH on dead groups is harmless.
                unsafe {
                    libc::killpg(pgid, libc::SIGTERM);
                }
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(100));

        for &pgid in pgids.iter() {
            if pgid > 0 {
                unsafe {
                    libc::killpg(pgid, libc::SIGKILL);
                }
            }
        }
    }
}

/// No-op on non-Unix platforms (no process group support).
#[cfg(not(unix))]
pub fn kill_all_children() {}

#[cfg(test)]
#[path = "child_guard.tests.rs"]
mod tests;
