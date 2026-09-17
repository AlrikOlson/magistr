//! What needs the human — the Needs-you dock's state.

use std::collections::VecDeque;

use magistr_workflow_contracts::{BlockerKind, Chunk};
use magistr_workflow_contracts::{NativeEvent, RoadmapFrame};

use super::dock_item::DockItem;
use super::engine_cmd::EngineCmd;
use super::permission::Permission;
use super::proposal_kind::ProposalKind;

/// The dock's cards, front first. Permission requests come from the
/// agent's session and sit ahead of everything else — the agent is
/// blocked on them; proposals and awaiting-human blockers fold from
/// roadmap events ([`Dock::apply`]) and leave when answered or when a
/// later event shows them gone.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dock {
    cards: VecDeque<DockItem>,
    // Answered cards keep their physical position until the engine replies.
    pending: Vec<EngineCmd>,
}

impl Dock {
    /// The front card — the one the keyboard answers.
    #[must_use]
    pub fn front(&self) -> Option<&DockItem> {
        self.cards().next()
    }

    /// The front card, to change in place — the Moment revealing the
    /// pictures its ask carries.
    pub fn front_mut(&mut self) -> Option<&mut DockItem> {
        let at = self.front_index()?;
        self.cards.get_mut(at)
    }

    /// Take the front card — it was answered.
    pub fn pop_front(&mut self) -> Option<DockItem> {
        let at = self.front_index()?;
        self.cards.remove(at)
    }

    /// Hide the front decision while its answer is in flight. Keeping the
    /// card in the queue preserves its place even if other answers follow.
    pub fn answer_front(&mut self, cmd: &EngineCmd) {
        if self.front().is_some_and(|card| answers(cmd, card)) {
            self.pending.push(cmd.clone());
        }
    }

    /// A refused answer is actionable again, in its original place.
    pub fn restore_answer(&mut self, cmd: &EngineCmd) {
        self.pending.retain(|pending| pending != cmd);
    }

    fn is_pending(&self, card: &DockItem) -> bool {
        self.pending.iter().any(|cmd| answers(cmd, card))
    }

    fn front_index(&self) -> Option<usize> {
        self.cards.iter().position(|card| !self.is_pending(card))
    }

    /// Take the front card when it is a permission — it was answered; a
    /// proposal or blocker at the front stays.
    pub fn pop_permission(&mut self) -> Option<Permission> {
        if matches!(self.cards.front(), Some(DockItem::Permission(_)))
            && let Some(DockItem::Permission(permission)) = self.cards.pop_front()
        {
            return Some(permission);
        }
        None
    }

    /// The cards in order, front first.
    pub fn cards(&self) -> impl Iterator<Item = &DockItem> {
        self.cards.iter().filter(|card| !self.is_pending(card))
    }

    /// How many cards wait.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cards().count()
    }

    /// Whether nothing needs the human.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.front().is_none()
    }

    /// Rows the dock takes: its divider line when empty; a card per row
    /// inside the needs-you panel's two border rows otherwise.
    #[must_use]
    pub fn rows(&self) -> usize {
        if self.is_empty() { 1 } else { self.len() + 2 }
    }

    /// How many permission requests wait for an answer.
    #[must_use]
    pub fn permissions_pending(&self) -> usize {
        self.cards
            .iter()
            .filter(|card| matches!(card, DockItem::Permission(_)))
            .count()
    }

    /// Take the diff-approval card whose title names `path` — the
    /// human approved that change on the Changes tab, so the card asking
    /// them to approve it has been answered. Only edit cards are
    /// candidates: a plain tool permission is never answered by a review
    /// of the journal.
    pub fn take_edit(&mut self, path: &str) -> Option<Permission> {
        self.take(|permission| permission.is_edit() && permission.title().contains(path))
    }

    /// Take the permission with `id`, wherever it waits: the transport
    /// answered it on its own — denied at its timeout, a lapse
    ///  — so the card would be dead, and a
    /// dead card is worse than none.
    pub fn remove_permission(&mut self, id: u64) -> Option<Permission> {
        self.take(|permission| permission.id() == id)
    }

    /// Take the first permission `pick` names.
    fn take(&mut self, pick: impl Fn(&Permission) -> bool) -> Option<Permission> {
        let at = self.cards.iter().position(|card| match card {
            DockItem::Permission(permission) => pick(permission),
            _ => false,
        })?;
        match self.cards.remove(at) {
            Some(DockItem::Permission(permission)) => Some(permission),
            _ => None,
        }
    }

    /// Queue a permission request: behind the pending permissions,
    /// ahead of every proposal and blocker — the agent is blocked on it.
    pub fn push_permission(&mut self, permission: Permission) {
        let at = self
            .cards
            .iter()
            .position(|card| !matches!(card, DockItem::Permission(_)))
            .unwrap_or(self.cards.len());
        self.cards.insert(at, DockItem::Permission(permission));
    }

    /// Fold one typed event in: a changed chunk upserts a card per open
    /// proposal and per awaiting-human blocker on it and drops the cards
    /// the record no longer carries; a completed or obsoleted chunk takes
    /// its cards away. Every other event is ignored.
    pub fn apply(&mut self, event: &NativeEvent) {
        let NativeEvent::Roadmap(frame) = event else {
            return;
        };
        let (chunk, ended) = match frame {
            RoadmapFrame::ChunkChanged { chunk } => (chunk, false),
            RoadmapFrame::ChunkCompleted { chunk } | RoadmapFrame::ChunkObsoleted { chunk } => {
                (chunk, true)
            }
            RoadmapFrame::ChunkAdded { .. } | RoadmapFrame::RefreshRecorded { .. } => return,
        };
        let id = chunk.id.as_str();
        // An event is authoritative: a still-open decision returns, a
        // resolved one leaves. Neither gets queued behind newer cards.
        self.pending.retain(|cmd| {
            !matches!(cmd,
                EngineCmd::ResolveProposal { chunk, .. } | EngineCmd::ClearBlocker { chunk }
                if chunk == id)
        });
        if ended {
            self.cards.retain(|card| match card {
                DockItem::Permission(_) => true,
                DockItem::Proposal { chunk, .. } | DockItem::Blocker { chunk, .. } => chunk != id,
            });
            return;
        }
        for kind in ProposalKind::ALL {
            match proposal(chunk, kind) {
                Some(card) => self.upsert(id, card),
                None => self.remove_proposal(id, kind),
            }
        }
        match blocker(chunk) {
            Some(card) => self.upsert(id, card),
            None => self.remove_blocker(id),
        }
    }

    /// Replace the card standing for the same thing, else queue it.
    fn upsert(&mut self, id: &str, card: DockItem) {
        let standing = self.cards.iter().position(|held| match (held, &card) {
            (DockItem::Proposal { chunk, kind, .. }, DockItem::Proposal { kind: new_kind, .. }) => {
                chunk == id && kind == new_kind
            }
            (DockItem::Blocker { chunk, .. }, DockItem::Blocker { .. }) => chunk == id,
            _ => false,
        });
        match standing {
            Some(at) => self.cards[at] = card,
            None => self.cards.push_back(card),
        }
    }

    fn remove_proposal(&mut self, id: &str, gone: ProposalKind) {
        self.cards.retain(|card| {
            !matches!(card, DockItem::Proposal { chunk, kind, .. } if chunk == id && *kind == gone)
        });
    }

    fn remove_blocker(&mut self, id: &str) {
        self.cards
            .retain(|card| !matches!(card, DockItem::Blocker { chunk, .. } if chunk == id));
    }
}

/// Whether the command answers this card's decision, independent of its copy.
fn answers(cmd: &EngineCmd, card: &DockItem) -> bool {
    match (cmd, card) {
        (
            EngineCmd::ResolveProposal { chunk, kind, .. },
            DockItem::Proposal {
                chunk: held,
                kind: held_kind,
                ..
            },
        ) => chunk == held && kind == held_kind,
        (EngineCmd::ClearBlocker { chunk }, DockItem::Blocker { chunk: held, .. }) => chunk == held,
        _ => false,
    }
}

/// The proposal card `chunk` carries for `kind`, if the record has one.
fn proposal(chunk: &Chunk, kind: ProposalKind) -> Option<DockItem> {
    let (suggestion, reason) = kind.on(chunk)?;
    Some(DockItem::Proposal {
        chunk: chunk.id.clone(),
        kind,
        suggestion,
        reason,
    })
}

/// The blocker card `chunk` carries, when it is blocked on a human.
fn blocker(chunk: &Chunk) -> Option<DockItem> {
    let blocked = chunk.blocked_by.as_ref()?;
    (blocked.kind == BlockerKind::AwaitingHuman).then(|| DockItem::Blocker {
        chunk: chunk.id.clone(),
        reason: blocked.reason.clone(),
    })
}

#[cfg(test)]
#[path = "dock.tests.rs"]
mod tests;
