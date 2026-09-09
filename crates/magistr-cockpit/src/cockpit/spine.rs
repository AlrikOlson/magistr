//! The rail's chain of checkpoints.

use magistr_engine::CheckpointReason;

use super::checkpoint::Checkpoint;

/// The session's checkpoint spine, oldest first, plus which row the
/// reader is previewing.
///
/// **The cockpit is the minter.** A permission card prints "cp-4 saved"
/// in the same fold that raises the card, so the name has to be decided
/// here, by the pure `update` — there is nothing to ask, and a name
/// arriving later could not be on the card. [`open`] therefore mints
/// and appends in one move and returns the name, which then travels
/// down as [`CheckpointCmd::Save`] and comes back as
/// [`CheckpointEvent::Saved`] against the same row.
///
/// **The chain is append-only.** Restoring to `cp-2` does not delete
/// `cp-3`: `cp-3` happened, and a reader who goes back to look at an
/// earlier state wants the later one still there to come forward to.
/// The mint never rewinds either, so one name means one state for the
/// life of the session.
///
/// [`open`]: Spine::open
/// [`CheckpointCmd::Save`]: super::CheckpointCmd::Save
/// [`CheckpointEvent::Saved`]: super::CheckpointEvent::Saved
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Spine {
    entries: Vec<Checkpoint>,
    minted: u32,
    selected: Option<usize>,
    receipts: u32,
    head_mark: u32,
}

impl Spine {
    /// Mint the next name, append a row under it and hand the name
    /// back so the caller can print it and ask the port for it.
    pub fn open(&mut self, reason: CheckpointReason, label: impl Into<String>) -> String {
        let id = self.mint();
        self.append(Checkpoint::asked(id.clone(), reason, label));
        id
    }

    /// Mint the row an iteration boundary lands on — `cp-N · iter N`,
    /// wearing the verdicts of the gates that iteration ran — and hand
    /// the name back so the port can be asked for it. The same counter
    /// as [`open`](Self::open): one name means one state whatever put
    /// it on the chain.
    pub fn open_boundary(&mut self, iteration: u32, gates: Vec<(String, bool)>) -> String {
        let id = self.mint();
        self.append(
            Checkpoint::asked(
                id.clone(),
                CheckpointReason::Iteration,
                format!("iter {iteration}"),
            )
            .with_gates(gates),
        );
        id
    }

    fn mint(&mut self) -> String {
        self.minted += 1;
        format!("cp-{}", self.minted)
    }

    fn append(&mut self, checkpoint: Checkpoint) {
        self.entries.push(checkpoint);
        // A new head starts its own count of what happened past it.
        self.head_mark = self.receipts;
    }

    /// A receipt landed — a tool call the session completed. The head
    /// row's `N receipts after cp-4` is the count taken past the newest
    /// checkpoint, so the ledger can say how much of the session is
    /// *not* inside any checkpoint's reach.
    pub const fn note_receipt(&mut self) {
        self.receipts = self.receipts.saturating_add(1);
    }

    /// The session's receipt counter — the ordinal the newest receipt
    /// was minted at, and what a completed call's card prints as
    /// `receipt #N`. One counter serves the card and the head row's
    /// `N receipts after cp-N`, so the two readings cannot disagree.
    #[must_use]
    pub const fn receipts(&self) -> u32 {
        self.receipts
    }

    /// Start the receipt counter at `n` — construction-time seeding for
    /// a session resumed from history (and the mock fixtures), before
    /// any checkpoint opens. Never called on a live fold: receipts
    /// arrive one [`Spine::note_receipt`] at a time.
    pub const fn seed_receipts(&mut self, n: u32) {
        self.receipts = n;
        self.head_mark = n;
    }

    /// Receipts taken since the newest checkpoint — `None` while the
    /// spine is empty, because "after" needs something to be after.
    #[must_use]
    pub fn receipts_after_head(&self) -> Option<u32> {
        self.head()
            .map(|_| self.receipts.saturating_sub(self.head_mark))
    }

    /// The port took `id`.
    pub fn saved(&mut self, id: &str, head: Option<String>, verified: bool) {
        if let Some(entry) = self.find_mut(id) {
            entry.save(head, verified);
        }
    }

    /// The port refused `id`.
    pub fn failed(&mut self, id: &str, error: &str) {
        if let Some(entry) = self.find_mut(id) {
            entry.fail(error);
        }
    }

    fn find_mut(&mut self, id: &str) -> Option<&mut Checkpoint> {
        self.entries.iter_mut().find(|entry| entry.id() == id)
    }

    /// The checkpoint `id` names.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Checkpoint> {
        self.entries.iter().find(|entry| entry.id() == id)
    }

    /// Every checkpoint, oldest first.
    #[must_use]
    pub fn entries(&self) -> &[Checkpoint] {
        &self.entries
    }

    /// The newest — the ledger head.
    #[must_use]
    pub fn head(&self) -> Option<&Checkpoint> {
        self.entries.last()
    }

    /// The newest checkpoint the port confirmed reproducible — the
    /// "last verified state" a review is scoped since.
    ///
    /// Newest *verified*, not the head: the head may be the checkpoint
    /// a risky ask minted moments ago and the port has not answered
    /// for, and scoping a review to a state nobody has confirmed would
    /// be scoping it to nothing. A spine whose rows are all unverified
    /// therefore answers `None` rather than the head, and the caller
    /// falls back to the journal.
    #[must_use]
    pub fn verified_head(&self) -> Option<&Checkpoint> {
        self.entries.iter().rev().find(|entry| entry.is_verified())
    }

    /// How many states the chain holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing has been checkpointed. An empty spine is what
    /// the rail draws nothing for, so a cockpit that never checkpointed
    /// is the cockpit it always was.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Preview the row at `index` — the third verb, and the one that
    /// changes nothing. A row already previewed is folded back, so the
    /// same click is its own undo.
    pub fn preview(&mut self, index: usize) {
        if index >= self.entries.len() {
            return;
        }
        self.selected = if self.selected == Some(index) {
            None
        } else {
            Some(index)
        };
    }

    /// The row being previewed.
    #[must_use]
    pub const fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// The checkpoint being previewed.
    #[must_use]
    pub fn previewed(&self) -> Option<&Checkpoint> {
        self.selected.and_then(|index| self.entries.get(index))
    }
}

#[cfg(test)]
#[path = "spine.tests.rs"]
mod tests;
