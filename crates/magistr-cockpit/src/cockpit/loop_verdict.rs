//! What the judge said about an iteration.

use crate::pixel::Icon;
use crate::theme::Icons;

/// One verdict the loop driver reported: the judge's answer in Claude
/// Code's three shapes — met, not yet, impossible — and its reason. The
/// driver has already reconciled it against the real gates before it
/// reaches the cockpit; this is what the strip and the stage show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoopVerdict {
    /// The judge said the chunk's acceptance is met.
    pub ok: bool,
    /// The judge said the acceptance cannot be met as written.
    pub impossible: bool,
    /// The judge's reason, verbatim.
    pub reason: String,
}

impl LoopVerdict {
    /// The verdict in one word: `met`, `not yet`, `impossible`.
    #[must_use]
    pub const fn word(&self) -> &'static str {
        if self.impossible {
            "impossible"
        } else if self.ok {
            "met"
        } else {
            "not yet"
        }
    }

    /// The strip's LED: the pass mark for met, the fail mark otherwise.
    #[must_use]
    pub const fn icon(&self) -> Icon {
        Icon::gate(Some(self.ok && !self.impossible))
    }

    /// The strip's LED: the pass glyph for met, the fail glyph otherwise.
    #[must_use]
    pub const fn glyph(&self, icons: &Icons) -> &'static str {
        icons.of(self.icon())
    }

    /// The word and the reason: `not yet — the tests are missing`; the
    /// word alone when the judge gave no reason.
    #[must_use]
    pub fn text(&self) -> String {
        if self.reason.trim().is_empty() {
            self.word().to_owned()
        } else {
            format!("{} — {}", self.word(), self.reason.trim())
        }
    }
}

#[cfg(test)]
#[path = "loop_verdict.tests.rs"]
mod tests;
