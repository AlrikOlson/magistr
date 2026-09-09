//! The window size of a tty, as cells.

use std::io;
use std::os::fd::AsFd;

use magistr_cockpit::cockpit::Size;

/// The tty's size from `TIOCGWINSZ`. A terminal that reports zero on
/// either axis (some CI ptys do) is taken as 80 × 24 so the cockpit can
/// still lay out.
pub(crate) fn winsize(fd: impl AsFd) -> io::Result<Size> {
    let ws = rustix::termios::tcgetwinsize(fd)?;
    Ok(if ws.ws_col == 0 || ws.ws_row == 0 {
        Size::new(80, 24)
    } else {
        Size::new(ws.ws_col, ws.ws_row)
    })
}
