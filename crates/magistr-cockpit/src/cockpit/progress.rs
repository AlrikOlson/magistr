//! What the terminal's own progress indicator is told.

use super::loop_status::LoopStatus;

/// The progress the terminal shows for us in its tab, taskbar or dock,
/// as ConEmu's OSC 9;4 names it: `OSC 9; 4; state; value ST`,
/// where the state is 0 (no bar), 1 (a fraction), 2 (a fraction, in the
/// error colour) or 3 (indeterminate). Ghostty, Windows Terminal,
/// iTerm2 3.6.6+, WezTerm and ConEmu honour it; nobody else is told
/// (the terminal runtime's `Env::shows_progress`).
///
/// This is the whole mapping from the iteration loop, and it is a pure
/// function of [`LoopStatus`] so the driver can emit it only when it
/// *changes*. Precedence is worst news first: a gate that failed shows
/// the error state even if another gate is still running, because that
/// failure is what the user in another window needs to come back for; a
/// gate still running shows indeterminate, since a gate has no fraction;
/// otherwise the loop's own `iteration/max` is the fraction. A loop that
/// ended, and a cockpit running no loop at all, show nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Progress {
    /// No bar: no loop, or the loop is over.
    #[default]
    Cleared,
    /// The loop is `n` percent through its iterations.
    Running(u8),
    /// A gate failed, at the same fraction — the error colour.
    Failed(u8),
    /// A gate is running and has no fraction to report.
    Indeterminate,
}

impl Progress {
    /// What the terminal should show for `status`.
    #[must_use]
    pub fn for_loop(status: Option<&LoopStatus>) -> Self {
        let Some(status) = status.filter(|status| status.ended().is_none()) else {
            return Self::Cleared;
        };
        let gates = status.gates();
        if gates.iter().any(|gate| gate.passed() == Some(false)) {
            Self::Failed(percent(status))
        } else if gates.iter().any(|gate| gate.passed().is_none()) {
            Self::Indeterminate
        } else {
            Self::Running(percent(status))
        }
    }

    /// Append the OSC 9;4 sequence for this state.
    ///
    /// The value field is always written, `0` where the state ignores it
    /// — every terminal on the allowlist accepts the four-parameter form,
    /// and one shape is one fewer thing to get wrong.
    pub fn write(self, out: &mut Vec<u8>) {
        let (state, value) = match self {
            Self::Cleared => (0, 0),
            Self::Running(percent) => (1, percent),
            Self::Failed(percent) => (2, percent),
            Self::Indeterminate => (3, 0),
        };
        out.extend_from_slice(format!("\x1b]9;4;{state};{value}\x1b\\").as_bytes());
    }
}

/// How far through its iterations the loop is, as the roadmap words it:
/// `iteration n/N` as a percentage, so the last iteration reads full.
/// A loop with no ceiling reports nothing rather than dividing by zero.
fn percent(status: &LoopStatus) -> u8 {
    let max = u64::from(status.max());
    if max == 0 {
        return 0;
    }
    let done = u64::from(status.iteration()).min(max);
    u8::try_from(done * 100 / max).unwrap_or(100)
}

#[cfg(test)]
#[path = "progress.tests.rs"]
mod tests;
