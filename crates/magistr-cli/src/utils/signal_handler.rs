//! SIGINT (Ctrl+C) handling: a graceful request first, an emergency exit
//! second.
//!
//! The first `SIGINT` requests [`Shutdown`]: owners stop admitting work,
//! cancel what is active, settle and release; the command finishes its
//! own settlement and exits [`Shutdown::EXIT_CODE`]. A second `SIGINT`
//! before that — or the settle deadline passing — is the emergency exit:
//! the terminal restored, the registered process groups killed, the same
//! exit code, and a line on stderr saying so. Without either path, Ctrl+C
//! would kill the process without running Drop impls, leaving the terminal
//! in mouse tracking mode. Once `ctrl_c` is used, tokio keeps capturing
//! later `SIGINT`s for the life of the process, so the second signal is
//! waited for explicitly.

use super::shutdown::Shutdown;

/// Install the two-phase SIGINT (Ctrl+C) handler.
pub fn install_signal_handler() {
    tokio::spawn(async {
        if tokio::signal::ctrl_c().await.is_err() {
            return;
        }
        Shutdown::request();
        eprintln!(
            "magistr: interrupted; settling (press Ctrl+C again to exit at once, {}s at most)",
            Shutdown::SETTLE_DEADLINE.as_secs()
        );
        let reason = tokio::select! {
            _ = tokio::signal::ctrl_c() => "interrupted again",
            () = tokio::time::sleep(Shutdown::SETTLE_DEADLINE) => "settlement deadline passed",
        };
        emergency_exit(reason);
    });
}

/// The way out that settles nothing: restore the terminal, kill the
/// registered process groups, say so, and exit.
fn emergency_exit(reason: &str) -> ! {
    super::child_guard::kill_all_children();
    eprintln!("magistr: emergency exit ({reason})");
    std::process::exit(Shutdown::EXIT_CODE);
}
