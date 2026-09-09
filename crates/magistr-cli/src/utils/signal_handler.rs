//! SIGINT (Ctrl+C) handler that restores the terminal before exit.
//!
//! Spawns a tokio task that waits for `ctrl_c()` and performs terminal cleanup.
//! Without this, Ctrl+C kills the process without running Drop impls, leaving
//! the terminal in mouse tracking mode.

/// Install a SIGINT (Ctrl+C) handler that restores the terminal before exit.
pub fn install_signal_handler() {
    tokio::spawn(async {
        if tokio::signal::ctrl_c().await.is_ok() {
            super::panic::emergency_restore_terminal();
            // Exit with standard SIGINT code (128 + 2)
            std::process::exit(130);
        }
    });
}
