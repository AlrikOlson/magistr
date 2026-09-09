//! What a fake terminal will send, and when.

use std::time::Duration;

use super::step::Step;
use magistr_cockpit::cockpit::Size;

/// A timeline of [`Step`]s. Times are measured from the fake input's
/// first read; steps are delivered in time order, ties in insertion order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Script {
    steps: Vec<(Duration, Step)>,
}

impl Script {
    /// An empty script: the terminal stays silent.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a step due at `at`.
    #[must_use]
    pub fn step(mut self, at: Duration, step: Step) -> Self {
        self.steps.push((at, step));
        self
    }

    /// Send `bytes` at `at`.
    #[must_use]
    pub fn bytes(self, at: Duration, bytes: impl Into<Vec<u8>>) -> Self {
        self.step(at, Step::Bytes(bytes.into()))
    }

    /// Type `text` at `at` (each character a key, as a terminal without
    /// the kitty protocol sends them).
    #[must_use]
    pub fn text(self, at: Duration, text: &str) -> Self {
        self.bytes(at, text.as_bytes())
    }

    /// Press and release the left mouse button at cell `(col, row)` at
    /// `at`, in SGR encoding.
    #[must_use]
    pub fn click(self, at: Duration, col: u16, row: u16) -> Self {
        let (x, y) = (u32::from(col) + 1, u32::from(row) + 1);
        self.bytes(at, format!("\x1b[<0;{x};{y}M\x1b[<0;{x};{y}m"))
    }

    /// Move the pointer over cell `(col, row)` at `at` with no button
    /// down, in SGR encoding — what makes the cockpit hover.
    #[must_use]
    pub fn mouse_move(self, at: Duration, col: u16, row: u16) -> Self {
        let (x, y) = (u32::from(col) + 1, u32::from(row) + 1);
        self.bytes(at, format!("\x1b[<35;{x};{y}M"))
    }

    /// Roll the wheel up one notch at cell `(col, row)` at `at`.
    #[must_use]
    pub fn scroll_up(self, at: Duration, col: u16, row: u16) -> Self {
        self.wheel(at, 64, col, row)
    }

    /// Roll the wheel down one notch at cell `(col, row)` at `at`.
    #[must_use]
    pub fn scroll_down(self, at: Duration, col: u16, row: u16) -> Self {
        self.wheel(at, 65, col, row)
    }

    fn wheel(self, at: Duration, button: u8, col: u16, row: u16) -> Self {
        let (x, y) = (u32::from(col) + 1, u32::from(row) + 1);
        self.bytes(at, format!("\x1b[<{button};{x};{y}M"))
    }

    /// Press PageUp at `at`.
    #[must_use]
    pub fn page_up(self, at: Duration) -> Self {
        self.bytes(at, b"\x1b[5~".as_slice())
    }

    /// Press PageDown at `at`.
    #[must_use]
    pub fn page_down(self, at: Duration) -> Self {
        self.bytes(at, b"\x1b[6~".as_slice())
    }

    /// The window becomes `size` at `at`.
    #[must_use]
    pub fn resize(self, at: Duration, size: Size) -> Self {
        self.step(at, Step::Resize(size))
    }

    /// The input closes at `at`.
    #[must_use]
    pub fn close(self, at: Duration) -> Self {
        self.step(at, Step::Close)
    }

    /// The replies a graphics-capable terminal gives the startup queries,
    /// at `at`: the kitty query answered `OK`, DA1 (which ends the probe
    /// window), the background colour, kitty keyboard flags 3 and a
    /// `10 × 20` px cell.
    #[must_use]
    pub fn startup_replies(self, at: Duration) -> Self {
        self.bytes(
            at,
            b"\x1b_Gi=31;OK\x1b\\\x1b[?62;4c\x1b]11;rgb:1e1e/1e1e/2e2e\x1b\\\x1b[?3u\x1b[6;20;10t"
                .as_slice(),
        )
    }

    /// The replies kitty's text sizing handshake gets from a terminal
    /// that renders a scaled run, at `at`: three cursor-position reports
    /// two columns apart. A script that never sends these is a terminal
    /// that swallowed the two `OSC 66` runs — the probe's default
    /// answer, and what every existing capture keeps showing.
    #[must_use]
    pub fn text_sizing_replies(self, at: Duration) -> Self {
        self.bytes(at, b"\x1b[1;1R\x1b[1;3R\x1b[1;5R".as_slice())
    }

    /// The replies a plain terminal gives: DA1 only.
    #[must_use]
    pub fn plain_replies(self, at: Duration) -> Self {
        self.bytes(at, b"\x1b[?62c".as_slice())
    }

    /// The steps in delivery order.
    #[must_use]
    pub fn into_steps(mut self) -> Vec<(Duration, Step)> {
        self.steps.sort_by_key(|(at, _)| *at);
        self.steps
    }

    /// Number of steps.
    #[must_use]
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Whether the script has no steps.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

#[cfg(test)]
#[path = "script.tests.rs"]
mod tests;
