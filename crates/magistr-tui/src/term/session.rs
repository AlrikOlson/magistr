//! Entering and leaving the full-screen session.

use magistr_cockpit::cockpit::Progress;

/// The escape sequences that take the terminal over for the cockpit and
/// restore it afterwards.
///
/// [`Session::enter`] switches to the alternate screen, hides the cursor,
/// turns on every mouse report (press, drag, motion) in SGR encoding,
/// bracketed paste and focus events, and pushes the kitty keyboard flags.
/// [`Session::leave`] undoes each in reverse order — the keyboard flags
/// are popped first, the alternate screen left last — after resetting the
/// pen, so nothing the cockpit set leaks into the shell.
///
/// A **flat** session ([`Session::flat`]) takes far
/// less: no alternate screen — which is what puts the transcript in the
/// terminal's own scrollback, where a screen reader and the mouse can
/// select it — no hidden cursor and no mouse reporting, which is only
/// noise for a reader. It keeps bracketed paste and the kitty keyboard
/// flags, because those are how the prompt line still works.
///
/// A session on a terminal that shows an OSC 9;4 progress bar
/// ([`Session::with_progress`]) also clears that bar on the way out, so a
/// crash never leaves the tab or the dock stuck part-full: the
/// [`PanicHook`](super::PanicHook) holds this session and writes the
/// leave sequence before the message prints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Session {
    keyboard_flags: u8,
    progress: bool,
    flat: bool,
}

impl Session {
    /// The cockpit's session: kitty flags 1 (disambiguate escape codes)
    /// and 2 (report event types).
    #[must_use]
    pub const fn cockpit() -> Self {
        Self {
            keyboard_flags: 3,
            progress: false,
            flat: false,
        }
    }

    /// The flat cockpit's session: the same kitty keyboard flags and
    /// bracketed paste, and nothing that takes the terminal over.
    #[must_use]
    pub const fn flat() -> Self {
        Self {
            keyboard_flags: 3,
            progress: false,
            flat: true,
        }
    }

    /// Whether this session leaves the terminal's own screen alone.
    #[must_use]
    pub const fn is_flat(self) -> bool {
        self.flat
    }

    /// Report progress to this terminal, and clear the bar on leave
    /// ([`Env::shows_progress`](crate::graphics::Env::shows_progress)
    /// decides). Off by default, so every capture of a bare
    /// [`Session::cockpit`] is byte-identical to what it always was.
    #[must_use]
    pub const fn with_progress(mut self, progress: bool) -> Self {
        self.progress = progress;
        self
    }

    /// Whether the cockpit may report progress to this terminal.
    #[must_use]
    pub const fn shows_progress(self) -> bool {
        self.progress
    }

    /// The kitty keyboard flags this session pushes.
    #[must_use]
    pub const fn keyboard_flags(self) -> u8 {
        self.keyboard_flags
    }

    /// Append the take-over sequence — everything but the paste and the
    /// keyboard flags left out for a flat session.
    pub fn enter(self, out: &mut Vec<u8>) {
        if !self.flat {
            out.extend_from_slice(b"\x1b[?1049h");
            out.extend_from_slice(b"\x1b[?25l");
            out.extend_from_slice(b"\x1b[?1016l\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1006h");
        }
        out.extend_from_slice(b"\x1b[?2004h");
        if !self.flat {
            out.extend_from_slice(b"\x1b[?1004h");
        }
        out.extend_from_slice(format!("\x1b[>{}u", self.keyboard_flags).as_bytes());
    }

    /// Append the restore sequence: exactly what [`Session::enter`] set,
    /// in reverse.
    pub fn leave(self, out: &mut Vec<u8>) {
        if self.progress {
            Progress::Cleared.write(out);
        }
        out.extend_from_slice(b"\x1b[0m");
        out.extend_from_slice(b"\x1b[<u");
        if !self.flat {
            out.extend_from_slice(b"\x1b[?1004l");
        }
        out.extend_from_slice(b"\x1b[?2004l");
        if !self.flat {
            // Also undo the optional mode enabled after the graphics probe.
            // Keeping this in Session makes panic cleanup restore it too.
            out.extend_from_slice(b"\x1b[?1016l\x1b[?1006l\x1b[?1003l\x1b[?1002l\x1b[?1000l");
            out.extend_from_slice(b"\x1b[?25h");
            out.extend_from_slice(b"\x1b[?1049l");
        }
    }

    /// Append the startup queries whose replies the decoder reports as
    /// [`Reply`](crate::input::Reply): the background colour (OSC 11),
    /// the kitty keyboard flags in effect (`CSI ? u`), one cell's size in
    /// pixels (`CSI 16 t`) and the text area's size in pixels
    /// (`CSI 14 t`, the fallback for terminals that ignore `16 t`).
    pub fn queries(self, out: &mut Vec<u8>) {
        out.extend_from_slice(b"\x1b]11;?\x1b\\");
        out.extend_from_slice(b"\x1b[?u");
        out.extend_from_slice(b"\x1b[16t");
        out.extend_from_slice(b"\x1b[14t");
    }
}

#[cfg(test)]
#[path = "session.tests.rs"]
mod tests;
