//! The high-water mark: what the flat cockpit has already said.

use super::flat_line::FlatLine;
use super::lines::{detail_lines, session_lines};
use magistr_cockpit::cockpit::Model;

/// How far into the flat rendering the terminal has been written, so a
/// frame emits only what settled since the last one.
///
/// [`Transcript::advance`] takes the **settled prefix** of
/// [`lines`](super::lines) — not every settled line — and returns the
/// part of it past the mark. Taking the prefix is what keeps the order a
/// reader hears the same as the order things happened: a note pushed
/// after a tool call that is still running waits for it, because it came
/// second. (And a call that never finishes cannot wedge the transcript,
/// because a turn that stops streaming settles whole.)
///
/// Nothing here can rewrite a line it has already returned. That is the
/// point — a screen reader has already read it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Transcript {
    emitted: usize,
    details: Vec<String>,
}

impl Transcript {
    /// A transcript that has said nothing yet.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            emitted: 0,
            details: Vec::new(),
        }
    }

    /// The lines that have settled since the last call, rendered, in
    /// order. Empty when nothing new has settled — which is most frames,
    /// and is why a flat cockpit writes nothing while it thinks.
    pub fn advance(&mut self, model: &Model) -> Vec<String> {
        let lines = session_lines(model);
        let settled = lines
            .iter()
            .position(|line| !line.is_settled())
            .unwrap_or(lines.len());
        let mut fresh = if settled > self.emitted {
            let fresh = lines[self.emitted..settled]
                .iter()
                .map(FlatLine::render)
                .collect::<Vec<_>>();
            self.emitted = settled;
            fresh
        } else {
            Vec::new()
        };
        let details = detail_lines(model)
            .iter()
            .map(FlatLine::render)
            .collect::<Vec<_>>();
        if details != self.details {
            fresh.extend(details.iter().cloned());
            self.details = details;
        }
        fresh
    }

    /// How many lines have been written out.
    #[must_use]
    pub const fn emitted(&self) -> usize {
        self.emitted
    }
}

#[cfg(test)]
#[path = "transcript.tests.rs"]
mod tests;
