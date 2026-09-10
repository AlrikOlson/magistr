//! Whether a process id still names a live process — the one question a
//! restarted loop can ask a provider runtime it no longer owns.

/// `Some(true)` when `pid` is alive (or exists but is not ours to signal),
/// `Some(false)` when no such process exists, `None` when the host cannot
/// say. A live answer can be a reused id; this is evidence, not proof.
#[must_use]
pub fn is_process_alive(pid: u32) -> Option<bool> {
    #[cfg(unix)]
    {
        let pid = i32::try_from(pid).ok().filter(|pid| *pid > 0)?;
        // SAFETY: signal 0 delivers nothing; kill only checks that the
        // process exists and that we may signal it. `pid` is positive, so
        // this never addresses a process group or every process.
        let answer = unsafe { libc::kill(pid, 0) };
        if answer == 0 {
            return Some(true);
        }
        match std::io::Error::last_os_error().raw_os_error() {
            Some(libc::EPERM) => Some(true),
            Some(libc::ESRCH) => Some(false),
            _ => None,
        }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        None
    }
}

#[cfg(test)]
#[path = "process_alive.tests.rs"]
mod tests;
