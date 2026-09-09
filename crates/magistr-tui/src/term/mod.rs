//! The terminal side of the cockpit: the session as bytes, the
//! [`Terminal`] the runtime writes to, the [`TerminalInput`] its input
//! thread reads from, and the real implementation over the process's
//! own file descriptors.
//!
//! [`Session`] is pure bytes. [`RealTerminal`] owns duplicates of stdin and
//! stdout, puts the tty in raw mode for as long as it lives (a [`RawMode`]
//! guard restores the saved termios in `Drop`, panics included), writes
//! frames, answers the window size and hands out a [`RealInput`] — a
//! `poll`-then-`read` with a timeout — for the [`InputThread`], which
//! decodes bytes into events, resolves a lone Escape after a quiet read
//! and reports a changed window size. No signal handler, no `unsafe`:
//! everything goes through rustix.
//!
//! Terminal integrity: [`OwnedStderr`] redirects the process's stderr into
//! an anonymous file for the life of the session, so no engine banner,
//! log line or panic message can scribble the alternate screen — the
//! text is replayed on the restored stderr after the leave sequence. The
//! [`PanicHook`] holds a [`Restorer`] for the session: a panic on any
//! thread writes the leave sequence, cooks the tty ([`TermiosRestore`])
//! and hands stderr back before the message prints.
//!
//! All of that is a Unix tty. Where there is none (Windows) the same
//! two names, [`RealTerminal`] and [`RealInput`], come from
//! [`unsupported`] and [`unsupported_input`]: opening the terminal fails
//! with `Unsupported` on the normal console before anything is painted,
//! and [`OwnedStderr::take`] refuses the same way, so every entry point
//! reports the missing backend instead of half-taking the screen.

pub mod input;
pub mod input_event;
pub mod input_thread;
pub mod owned_stderr;
pub mod panic_hook;
#[cfg(unix)]
pub mod raw_mode;
#[cfg(unix)]
pub mod real;
#[cfg(unix)]
pub mod real_input;
pub mod restorer;
pub mod session;
pub mod terminal;
#[cfg(unix)]
pub mod termios_restore;
#[cfg(not(unix))]
pub mod unsupported;
#[cfg(not(unix))]
pub mod unsupported_input;
#[cfg(unix)]
mod winsize;

pub use input::TerminalInput;
pub use input_event::InputEvent;
pub use input_thread::InputThread;
pub use owned_stderr::OwnedStderr;
pub use panic_hook::PanicHook;
#[cfg(unix)]
pub use raw_mode::RawMode;
#[cfg(unix)]
pub use real::RealTerminal;
#[cfg(unix)]
pub use real_input::RealInput;
pub use restorer::Restorer;
pub use session::Session;
pub use terminal::Terminal;
#[cfg(unix)]
pub use termios_restore::TermiosRestore;
#[cfg(not(unix))]
pub use unsupported::RealTerminal;
#[cfg(not(unix))]
pub use unsupported_input::RealInput;
