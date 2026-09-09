//! A permission request waiting for the user's answer.

use crate::agent::{AskPlan, ImageBlock, PermissionChoice, PermissionKind};

/// One request the agent made, as the strip shows it: which request,
/// what it wants to run, and the options offered. The answer is one of
/// the options' ids.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Permission {
    id: u64,
    title: String,
    choices: Vec<PermissionChoice>,
    edit: bool,
    checkpoint: Option<String>,
    plan: Option<AskPlan>,
    revealed: bool,
}

impl Permission {
    /// Request `id` to run `title`, offering `choices`.
    #[must_use]
    pub fn new(id: u64, title: impl Into<String>, choices: Vec<PermissionChoice>) -> Self {
        Self {
            id,
            title: title.into(),
            choices,
            edit: false,
            checkpoint: None,
            plan: None,
            revealed: false,
        }
    }

    /// The same request, about `plan` — which is what raises the
    /// design's Moment instead of the dock's one-line strip.
    #[must_use]
    pub fn about(mut self, plan: AskPlan) -> Self {
        self.plan = Some(plan);
        self
    }

    /// The plan the request is about, when it is about one.
    #[must_use]
    pub const fn plan(&self) -> Option<&AskPlan> {
        self.plan.as_ref()
    }

    /// Whether this ask is a plan approval — the Moment's own
    /// predicate, so no call site re-derives it from an empty step
    /// list.
    #[must_use]
    pub const fn is_plan(&self) -> bool {
        self.plan.is_some()
    }

    /// The plan's steps, in order; empty for an ask with no plan.
    pub fn steps(&self) -> impl Iterator<Item = &str> {
        self.plan
            .iter()
            .flat_map(|plan| plan.steps.iter().map(String::as_str))
    }

    /// What a session-wide allow would cover, in the reader's words.
    #[must_use]
    pub fn scope(&self) -> Option<&str> {
        self.plan.as_ref().and_then(|plan| plan.scope.as_deref())
    }

    /// The pictures the call carries and the reader has not asked for
    /// yet. Empty once they have been shown, so the card's *Show
    /// images* button is offered exactly while there is something left
    /// to show.
    #[must_use]
    pub fn hidden_images(&self) -> &[ImageBlock] {
        if self.revealed {
            return &[];
        }
        self.plan.as_ref().map_or(&[], |plan| &plan.images)
    }

    /// Mark the pictures shown — the reader pressed `space` or the
    /// card's button — and hand back the blocks to put on the feed.
    ///
    /// Taking them by value rather than leaving the card to re-offer
    /// them is what makes the reveal idempotent: a second press has
    /// nothing to hand over and the button is already gone.
    pub fn reveal_images(&mut self) -> Vec<ImageBlock> {
        if self.revealed {
            return Vec::new();
        }
        self.revealed = true;
        self.plan
            .as_ref()
            .map(|plan| plan.images.clone())
            .unwrap_or_default()
    }

    /// Name the checkpoint saved before this request was raised.
    ///
    /// A risky allow that names the
    /// state it can be undone to is a different decision from one that
    /// does not. The name is minted by the spine in the same fold that
    /// pushes this card, so the card and the rail cannot disagree — see
    /// [`Spine::open`](super::Spine::open).
    #[must_use]
    pub fn at_checkpoint(mut self, id: impl Into<String>) -> Self {
        self.checkpoint = Some(id.into());
        self
    }

    /// The checkpoint saved before this request, when one was.
    #[must_use]
    pub fn checkpoint(&self) -> Option<&str> {
        self.checkpoint.as_deref()
    }

    /// Mark the request as an edit — the dock shows it as the
    /// diff-approval card.
    #[must_use]
    pub const fn edit(mut self) -> Self {
        self.edit = true;
        self
    }

    /// Whether the request is an edit (a diff approval).
    #[must_use]
    pub const fn is_edit(&self) -> bool {
        self.edit
    }

    /// The request's id, session-unique.
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    /// What the agent wants to run.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The option id for `kind`, when the agent offers one.
    #[must_use]
    pub fn choice(&self, kind: PermissionKind) -> Option<&str> {
        self.choices
            .iter()
            .find(|choice| choice.kind == kind)
            .map(|choice| choice.option_id.as_str())
    }

    /// The buttons to show: one per offered kind, in
    /// [`PermissionKind::ALL`] order, each with the option id it answers.
    pub fn buttons(&self) -> impl Iterator<Item = (PermissionKind, &str)> {
        PermissionKind::ALL
            .into_iter()
            .filter_map(|kind| self.choice(kind).map(|id| (kind, id)))
    }

    /// The plain allow: once if offered, else for the session.
    #[must_use]
    pub fn allow(&self) -> Option<&str> {
        self.choice(PermissionKind::AllowOnce)
            .or_else(|| self.choice(PermissionKind::AllowAlways))
    }

    /// The plain deny: once if offered, else for the session.
    #[must_use]
    pub fn deny(&self) -> Option<&str> {
        self.choice(PermissionKind::RejectOnce)
            .or_else(|| self.choice(PermissionKind::RejectAlways))
    }
}

#[cfg(test)]
#[path = "permission.tests.rs"]
mod tests;
