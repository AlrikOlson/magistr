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
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, STILL_ACTIVE};
        use windows_sys::Win32::System::Threading::{
            GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        if pid == 0 {
            return None;
        }
        // SAFETY: OpenProcess takes plain values and returns a handle or null;
        // the handle is closed below on every path that received one.
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            let error = std::io::Error::last_os_error();
            return match error.raw_os_error() {
                // Exists, but is not ours to inspect: alive for our purposes.
                Some(code) if code == ERROR_ACCESS_DENIED as i32 => Some(true),
                // ERROR_INVALID_PARAMETER: no such process.
                Some(87) => Some(false),
                _ => None,
            };
        }
        let mut code = 0_u32;
        // SAFETY: `handle` is a valid open process handle and `code` outlives
        // the call that writes it.
        let answered = unsafe { GetExitCodeProcess(handle, &mut code) } != 0;
        // SAFETY: closing the handle this function opened.
        unsafe { CloseHandle(handle) };
        if !answered {
            return None;
        }
        Some(code == STILL_ACTIVE as u32)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        None
    }
}

#[cfg(test)]
#[path = "process_alive.tests.rs"]
mod tests;
