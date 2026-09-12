use std::io;
use std::time::Duration;

use crate::ChildRegistry;

const POLL: Duration = Duration::from_millis(10);
pub(crate) const CLEANUP_TIMEOUT: Duration = Duration::from_secs(2);

/// Ownership of an isolated Unix process group and its hard-exit registration.
/// A dropped guard requests SIGKILL synchronously and retires membership only
/// after the group disappears. An incomplete cleanup stays registered.
pub struct ProcessGroup {
    pgid: Option<i32>,
}

impl ProcessGroup {
    /// Adopt the group created for a newly spawned child.
    ///
    /// # Safety
    /// On Unix, `pid` must name a freshly spawned child in its own process
    /// group whose PGID equals its PID. There must be only one owning guard.
    pub unsafe fn from_spawned_pid(pid: u32) -> Self {
        #[cfg(unix)]
        let pgid = i32::try_from(pid).ok().filter(|&id| id > 1);
        #[cfg(not(unix))]
        let pgid = {
            let _ = pid;
            None
        };
        if let Some(pgid) = pgid {
            ChildRegistry::register(pgid);
        }
        Self { pgid }
    }

    /// The still-owned group, or zero after settlement/on non-Unix.
    #[must_use]
    pub fn id(&self) -> i32 {
        self.pgid.unwrap_or(0)
    }

    /// Request immediate group termination without claiming it has finished.
    pub fn kill(&self) -> io::Result<()> {
        #[cfg(unix)]
        return self.signal(libc::SIGKILL);
        #[cfg(not(unix))]
        Ok(())
    }

    #[cfg(unix)]
    pub(crate) fn signal(&self, signal: i32) -> io::Result<()> {
        let Some(pgid) = self.pgid else {
            return Ok(());
        };
        // SAFETY: this guard exclusively owns the group adopted at spawn.
        if unsafe { libc::killpg(pgid, signal) } == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            Ok(())
        } else {
            Err(error)
        }
    }

    /// Probe group disappearance; successful signal delivery is insufficient.
    pub fn is_settled(&mut self) -> io::Result<bool> {
        if let Some(pgid) = self.pgid {
            if !group_gone(pgid)? {
                return Ok(false);
            }
            ChildRegistry::unregister(pgid);
            self.pgid = None;
        }
        Ok(true)
    }

    /// Await disappearance after the caller has killed/reaped the child owner.
    /// The deadline bounds even groups that cannot be killed or reaped.
    pub async fn wait_for_settlement(&mut self, timeout: Duration) -> io::Result<()> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if self.is_settled()? {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "process group cleanup incomplete",
                ));
            }
            tokio::time::sleep(POLL).await;
        }
    }
}

fn group_gone(pgid: i32) -> io::Result<bool> {
    #[cfg(unix)]
    {
        // SAFETY: signal zero only probes this owned group.
        if unsafe { libc::killpg(pgid, 0) } == 0 {
            return Ok(false);
        }
        let error = io::Error::last_os_error();
        match error.raw_os_error() {
            Some(libc::ESRCH) => Ok(true),
            // A group with only exiting/zombie members can transiently be
            // unsignalable on Darwin. Lack of permission is not disappearance:
            // retain ownership and let the cleanup deadline bound the probe.
            Some(libc::EPERM) => Ok(false),
            _ => Err(error),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = pgid;
        Ok(true)
    }
}

impl Drop for ProcessGroup {
    fn drop(&mut self) {
        let _ = self.kill();
        let Some(pgid) = self.pgid.take() else {
            return;
        };
        if group_gone(pgid).unwrap_or(false) {
            ChildRegistry::unregister(pgid);
            return;
        }
        // Reaping belongs to the child owner. This small fallback does not
        // require a live async runtime, including during panic unwinding.
        let _ = std::thread::Builder::new()
            .name("process-settlement".into())
            .spawn(move || {
                let deadline = std::time::Instant::now() + CLEANUP_TIMEOUT;
                loop {
                    if group_gone(pgid).unwrap_or(false) {
                        ChildRegistry::unregister(pgid);
                        break;
                    }
                    let interval = if std::time::Instant::now() >= deadline {
                        Duration::from_secs(1)
                    } else {
                        POLL
                    };
                    std::thread::sleep(interval);
                }
            });
    }
}
