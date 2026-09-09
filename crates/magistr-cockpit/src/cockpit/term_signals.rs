//! What a program tells its terminal past the screen.

use super::progress::Progress;

/// The program's own signals: the operating-system
/// commands the screen parser does not interpret, read as they pass —
/// today the ConEmu progress report, `OSC 9; 4; state; percent`,
/// the same vocabulary the loop writes to the terminal magistr runs in
/// ([`Progress`]). State `0` clears, `1` is a fraction, `2` the
/// fraction in the error colour, `3` indeterminate; `4` (a paused or
/// warning bar, where a terminal draws one) is read as the fraction —
/// the strip has no third hue. A report the parser cannot read leaves
/// the last one standing.
///
/// Held by the [`Term`](super::term::Term)'s parser and replayed with
/// its bytes, so a cloned term reports what the original does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TermSignals {
    progress: Progress,
}

impl TermSignals {
    /// The progress the program last reported.
    #[must_use]
    pub const fn progress(&self) -> Progress {
        self.progress
    }

    /// The progress a `9;4` report's trailing parameters name, when
    /// they name one.
    fn read_progress(rest: &[&[u8]]) -> Option<Progress> {
        let state: u8 = std::str::from_utf8(rest.first()?).ok()?.parse().ok()?;
        let value = rest
            .get(1)
            .and_then(|value| std::str::from_utf8(value).ok())
            .and_then(|value| value.parse::<u8>().ok())
            .unwrap_or(0)
            .min(100);
        Some(match state {
            0 => Progress::Cleared,
            1 | 4 => Progress::Running(value),
            2 => Progress::Failed(value),
            3 => Progress::Indeterminate,
            _ => return None,
        })
    }
}

impl vt100::Callbacks for TermSignals {
    fn unhandled_osc(&mut self, _: &mut vt100::Screen, params: &[&[u8]]) {
        if let [b"9", b"4", rest @ ..] = params
            && let Some(progress) = Self::read_progress(rest)
        {
            self.progress = progress;
        }
    }
}

#[cfg(test)]
#[path = "term_signals.tests.rs"]
mod tests;
