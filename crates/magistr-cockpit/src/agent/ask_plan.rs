//! The plan an ask carries, when it asks about one.

use super::image_block::ImageBlock;

/// What turns a permission request into the design's *Moment*: the plan
/// the agent wants approved, the pictures the call brought with it, and
/// the scope a session-wide allow would cover.
///
/// One field on [`AgentEvent::PermissionRequest`] rather than three,
/// for two reasons. Every construction site would otherwise have to
/// default three values that only ever travel together; and a scope
/// with no steps to print it beside would be a promise the card has
/// nowhere to keep — `⇧A · ALWAYS ALLOW (GOLDENS)` is a line *on the
/// plan card*, so it lives where the plan does.
///
/// An ask with no plan is the ordinary tool permission the Needs-you
/// dock has always shown, unchanged.
///
/// [`AgentEvent::PermissionRequest`]: super::AgentEvent::PermissionRequest
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AskPlan {
    /// The plan's steps, in order — the card numbers them.
    pub steps: Vec<String>,
    /// The pictures the call carries, shown when the reader asks for
    /// them (`space`). Empty means the card offers no *Show images*
    /// button at all, rather than one that reveals nothing.
    pub images: Vec<ImageBlock>,
    /// What a session-wide allow would cover, in the reader's words —
    /// the `(goldens)` of `⇧A · ALWAYS ALLOW (GOLDENS)`. `None` prints
    /// the offer without a scope rather than a placeholder one.
    pub scope: Option<String>,
}

impl AskPlan {
    /// A plan of `steps`, with no pictures and no named scope.
    #[must_use]
    pub fn new<S: Into<String>>(steps: impl IntoIterator<Item = S>) -> Self {
        Self {
            steps: steps.into_iter().map(Into::into).collect(),
            images: Vec::new(),
            scope: None,
        }
    }

    /// The same plan, carrying `images`.
    #[must_use]
    pub fn with_images(mut self, images: Vec<ImageBlock>) -> Self {
        self.images = images;
        self
    }

    /// The same plan, naming what a session-wide allow would cover.
    #[must_use]
    pub fn scoped(mut self, scope: impl Into<String>) -> Self {
        self.scope = Some(scope.into());
        self
    }
}
