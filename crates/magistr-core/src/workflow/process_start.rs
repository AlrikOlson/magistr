//! The start identity of a process: what tells one holder of a pid from
//! the next, since the host hands ids out again once a process is gone.

/// An opaque identity of when `pid` started, as the host reports it —
/// seconds since the epoch on macOS, clock ticks since boot on Linux.
/// Stable for the life of the process and never shared with a later
/// holder of the same id on the same boot. `None` when the process does
/// not exist, when it is not ours to inspect, or when the host offers no
/// such identity (other platforms): the caller then has only the id.
#[must_use]
pub fn process_start(pid: u32) -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        let pid = i32::try_from(pid).ok().filter(|pid| *pid > 0)?;
        // SAFETY: the buffer is a zeroed `proc_bsdinfo` and its size is
        // passed with it; the kernel writes at most that many bytes.
        let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
        let size = i32::try_from(std::mem::size_of::<libc::proc_bsdinfo>()).ok()?;
        let written = unsafe {
            libc::proc_pidinfo(
                pid,
                libc::PROC_PIDTBSDINFO,
                0,
                std::ptr::from_mut(&mut info).cast(),
                size,
            )
        };
        (written == size).then_some(info.pbi_start_tvsec)
    }
    #[cfg(target_os = "linux")]
    {
        // `/proc/<pid>/stat`: `pid (comm) state ppid ...`; the command may
        // hold spaces and parentheses, so fields are counted after the last
        // `)`. `starttime` is the 22nd field of the line, the 20th after it.
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        let after = &stat[stat.rfind(')')? + 1..];
        after
            .split_whitespace()
            .nth(19)
            .and_then(|ticks| ticks.parse().ok())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = pid;
        None
    }
}
