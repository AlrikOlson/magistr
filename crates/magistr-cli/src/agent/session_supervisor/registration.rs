//! Child registry membership follows the actual transport owner.

/// The transport kills its process group; this guard removes the registry entry
/// when its actual owner exits, including startup failure and task abortion.
pub(super) struct Registration {
    #[cfg(unix)]
    pgid: i32,
}

impl Registration {
    pub(super) fn new(pgid: i32) -> Self {
        #[cfg(not(unix))]
        let _ = pgid;
        Self {
            #[cfg(unix)]
            pgid,
        }
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        #[cfg(unix)]
        if self.pgid > 0 {
            crate::utils::child_guard::unregister_child_pgid(self.pgid);
        }
    }
}
