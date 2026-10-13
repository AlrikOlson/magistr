//! The shared hard-exit registry; membership is normally owned by ProcessGroup.
use std::sync::Mutex;

static PGIDS: Mutex<Vec<i32>> = Mutex::new(Vec::new());
#[cfg(windows)]
static JOBS: Mutex<std::collections::BTreeMap<i32, std::os::windows::io::OwnedHandle>> =
    Mutex::new(std::collections::BTreeMap::new());

/// Process groups retained for emergency host shutdown.
pub struct ChildRegistry;

impl ChildRegistry {
    #[cfg(windows)]
    pub(crate) fn register_job(
        pgid: i32,
        job: &std::os::windows::io::OwnedHandle,
    ) -> std::io::Result<()> {
        let handle = job.try_clone()?;
        JOBS.lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(pgid, handle);
        Self::register(pgid);
        Ok(())
    }
    /// Register a group created by a host-owned spawn.
    pub fn register(pgid: i32) {
        if pgid <= 1 {
            return;
        }
        let mut groups = PGIDS.lock().unwrap_or_else(|error| error.into_inner());
        if !groups.contains(&pgid) {
            groups.push(pgid);
        }
    }

    /// Remove a group after confirmed settlement.
    pub fn unregister(pgid: i32) {
        #[cfg(windows)]
        JOBS.lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&pgid);
        PGIDS
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .retain(|&id| id != pgid);
    }

    /// Whether the group is still retained for emergency cleanup.
    #[must_use]
    pub fn contains(pgid: i32) -> bool {
        PGIDS
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .contains(&pgid)
    }

    /// Signal all registered groups during host shutdown.
    /// This requests termination; it does not claim settlement.
    pub fn kill_all() {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            let jobs = JOBS.lock().unwrap_or_else(|error| error.into_inner());
            for job in jobs.values() {
                // SAFETY: the registry retains only live, host-owned job handles.
                unsafe {
                    windows_sys::Win32::System::JobObjects::TerminateJobObject(
                        job.as_raw_handle(),
                        1,
                    );
                }
            }
        }
        #[cfg(unix)]
        {
            let groups = PGIDS.lock().unwrap_or_else(|error| error.into_inner());
            for signal in [libc::SIGTERM, libc::SIGKILL] {
                for &pgid in groups.iter() {
                    // SAFETY: registry entries name isolated groups owned by the host.
                    unsafe {
                        libc::killpg(pgid, signal);
                    }
                }
                if signal == libc::SIGTERM {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        }
    }
}
