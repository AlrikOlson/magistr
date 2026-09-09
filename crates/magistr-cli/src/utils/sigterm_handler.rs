//! SIGTERM handler that restores the terminal before exit.
//!
//! SIGTERM is sent by `kill` (default signal), process managers, and container
//! runtimes. Without this handler the terminal is left in raw mode.

/// Install a SIGTERM handler that restores the terminal before exit.
#[cfg(unix)]
pub fn install_sigterm_handler() {
    tokio::spawn(async {
        use tokio::signal::unix::{SignalKind, signal};
        if let Ok(mut sig) = signal(SignalKind::terminate()) {
            sig.recv().await;
            super::panic::emergency_restore_terminal();
            // Exit with standard SIGTERM code (128 + 15)
            std::process::exit(143);
        }
    });
}
