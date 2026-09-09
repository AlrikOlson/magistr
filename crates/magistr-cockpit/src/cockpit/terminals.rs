//! The programs' terminals, by id.

use std::collections::BTreeMap;

use super::term::Term;

/// Every terminal a program is writing to — the agent's, keyed by the
/// id `terminal/create` gave it; a gate's, keyed by the gate's name —
/// each as the [`Term`] its bytes make, for the pane that paints it.
/// Bytes may land before the tool call that embeds the terminal names
/// it, so they are kept here by id rather than on a line.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Terminals {
    by_id: BTreeMap<String, Term>,
}

impl Terminals {
    /// Append one chunk to `id`'s terminal, written on `tick`.
    pub fn push(&mut self, id: &str, bytes: &[u8], tick: u64) {
        self.by_id
            .entry(id.to_owned())
            .or_default()
            .push(bytes, tick);
    }

    /// The bytes `id` has written, when it has written anything —
    /// the newest [`Term::KEEP`] of them.
    #[must_use]
    pub fn bytes(&self, id: &str) -> Option<&[u8]> {
        self.by_id.get(id).map(Term::bytes)
    }

    /// `id`'s terminal, when it has written anything.
    #[must_use]
    pub fn term(&self, id: &str) -> Option<&Term> {
        self.by_id.get(id)
    }

    /// `id`'s terminal, to scroll.
    pub fn term_mut(&mut self, id: &str) -> Option<&mut Term> {
        self.by_id.get_mut(id)
    }

    /// `id`'s program ended: its screen holds still. A terminal that
    /// never wrote anything is not created for it.
    pub fn finish(&mut self, id: &str) {
        if let Some(term) = self.by_id.get_mut(id) {
            term.finish();
        }
    }

    /// The reader killed `id`'s program: its screen
    /// holds still, marked as the human's doing. Returns whether the
    /// terminal exists — nothing is created for one that never wrote.
    pub fn kill(&mut self, id: &str) -> bool {
        match self.by_id.get_mut(id) {
            Some(term) => {
                term.kill();
                true
            }
            None => false,
        }
    }

    /// Every scrolled terminal back on its tail. Returns whether any
    /// moved.
    pub fn follow_all(&mut self) -> bool {
        // Every terminal is visited — `any` would stop at the first
        // that moved and leave the rest held.
        let mut moved = false;
        for term in self.by_id.values_mut() {
            moved |= term.follow();
        }
        moved
    }
}

#[cfg(test)]
#[path = "terminals.tests.rs"]
mod tests;
