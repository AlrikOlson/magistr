//! Panic hook that restores the terminal before printing the backtrace.
//!
//! Without this, a panic during a cockpit session leaves the terminal with
//! mouse tracking still active, flooding escape sequences on mouse movement.

use std::io::Write;

use color_eyre::eyre;

/// Everything the cockpit's take-over sequence enables, disabled again:
/// mouse tracking (1000/1002/1003, SGR 1006, URXVT 1015), bracketed paste
/// (2004), focus reporting (1004), the kitty keyboard protocol pop, the
/// alternate screen (1049) and the cursor. Raw-mode termios is owned by the
/// cockpit's `RawMode` guard (crates/magistr-tui), which restores it on
/// drop — panics included; this hook only clears the screen modes, which
/// survive the process otherwise.
const RESTORE_SEQUENCE: &[u8] = b"\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l\x1b[?1015l\
\x1b[?2004l\x1b[?1004l\x1b[<u\x1b[?1049l\x1b[?25h";

/// Best-effort terminal restore: disable mouse tracking, paste/focus
/// reporting, KKP and the alt screen, and re-show the cursor. Used by the
/// panic hook and signal handlers.
///
/// All operations ignore errors — this runs in crash/signal paths where
/// partial restore is better than none.
pub(crate) fn emergency_restore_terminal() {
    super::child_guard::kill_all_children();
    let mut stdout = std::io::stdout();
    let _ = stdout.write_all(RESTORE_SEQUENCE);
    let _ = stdout.flush();
}

/// Install a panic hook that restores the terminal before printing the backtrace.
pub fn install_panic_hook() -> eyre::Result<()> {
    color_eyre::install()?;

    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        emergency_restore_terminal();
        default_hook(info);
    }));

    Ok(())
}

#[cfg(test)]
#[path = "panic.tests.rs"]
mod tests;
