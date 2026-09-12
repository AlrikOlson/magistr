//! A worker lives exactly as long as the owner that spawned it.
//!
//! The supervisor kills its worker group on every orderly path. A killed owner
//! runs no path at all, and macOS offers no parent-death signal, so the worker
//! watches for itself: once it is re-parented it ends, taking its own sandbox
//! children with it. Polling costs one system call per interval.

/// Exit this process once its parent is gone. `on_exit` runs first, so the
/// sandbox pool can reap its own children before the worker ends. On
/// platforms without a parent id the watch is a no-op.
pub(super) fn watch_parent(on_exit: fn()) {
    #[cfg(unix)]
    {
        use std::time::Duration;

        /// How often the worker checks that its owner is still its parent.
        const INTERVAL: Duration = Duration::from_millis(250);

        // SAFETY: getppid takes no arguments and cannot fail.
        let parent = unsafe { libc::getppid() };
        std::thread::Builder::new()
            .name("magistr-worker-parent-watch".into())
            .spawn(move || {
                loop {
                    std::thread::sleep(INTERVAL);
                    // SAFETY: as above.
                    if unsafe { libc::getppid() } != parent {
                        on_exit();
                        std::process::exit(0);
                    }
                }
            })
            .ok();
    }
    #[cfg(not(unix))]
    {
        let _ = on_exit;
    }
}
