//! One card on the Needs-you dock.

use super::permission::Permission;
use super::proposal_kind::ProposalKind;
use crate::pixel::Icon;
use crate::theme::Icons;

/// Something that needs the human: a typed card on the dock, in the
/// order it arrived.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DockItem {
    /// A tool wants to run; an edit request is the diff-approval card.
    Permission(Permission),
    /// An open roadmap proposal on a chunk: accept it or reject it.
    Proposal {
        /// The chunk the proposal is on.
        chunk: String,
        /// Which field it wants to change.
        kind: ProposalKind,
        /// The suggested value, as the card shows it.
        suggestion: String,
        /// Why, as the proposer recorded it.
        reason: String,
    },
    /// A chunk blocked awaiting a human decision: clear the blocker.
    Blocker {
        /// The blocked chunk.
        chunk: String,
        /// What it waits on.
        reason: String,
    },
}

impl DockItem {
    /// The card's one-line text, without its buttons: its mark, then
    /// the words after it.
    #[must_use]
    pub fn line(&self) -> (Icon, String) {
        match self {
            Self::Permission(p) if p.is_edit() => (
                Icon::Edit,
                saved(format!("{} — approve the diff", p.title()), p),
            ),
            Self::Permission(p) => (
                Icon::Warning,
                saved(format!("{} needs permission", p.title()), p),
            ),
            Self::Proposal {
                chunk,
                kind,
                suggestion,
                reason,
            } => (
                Icon::Signal,
                format!("{chunk}: {} → {suggestion} ({reason})", kind.label()),
            ),
            Self::Blocker { chunk, reason } => {
                (Icon::Blocker, format!("{chunk} blocked: {reason}"))
            }
        }
    }

    /// The card's one-line text, without its buttons, in `icons`'
    /// vocabulary.
    #[must_use]
    pub fn text(&self, icons: &Icons) -> String {
        let (icon, rest) = self.line();
        format!("{} {rest}", icons.of(icon))
    }
}

/// A risky allow names the state it can be undone to.
/// A request no checkpoint was taken before reads exactly as it always
/// did, which is what keeps every card that is not risky unmoved.
fn saved(text: String, permission: &Permission) -> String {
    match permission.checkpoint() {
        Some(id) => format!("{text} · {id} saved"),
        None => text,
    }
}

#[cfg(test)]
#[path = "dock_item.tests.rs"]
mod tests;
