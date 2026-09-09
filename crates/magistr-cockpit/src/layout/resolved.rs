//! The result of a layout pass.

use super::extent::Extent;
use super::placed::Placed;
use super::px_rect::PxRect;
use super::rect::Rect;
use crate::widget_id::WidgetId;

/// Every node's rect in tree order (parents before children, siblings in
/// layout order), so painting in order draws containers under their
/// contents.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Resolved {
    placed: Vec<Placed>,
}

impl Resolved {
    pub(crate) fn push(&mut self, placed: Placed) {
        self.placed.push(placed);
    }

    /// Every placed node, root first.
    #[must_use]
    pub fn placed(&self) -> &[Placed] {
        &self.placed
    }

    /// The rect of the first node carrying `id`.
    #[must_use]
    pub fn rect(&self, id: WidgetId) -> Option<Rect> {
        self.placed
            .iter()
            .find(|placed| placed.id == Some(id))
            .map(|placed| placed.rect)
    }

    /// The box of the first node carrying `id` in the engine's own
    /// units — the native painter's reading of
    /// [`rect`](Resolved::rect).
    #[must_use]
    pub fn px(&self, id: WidgetId) -> Option<PxRect> {
        self.placed
            .iter()
            .find(|placed| placed.id == Some(id))
            .map(|placed| placed.px)
    }

    /// The content extent of the first node carrying `id` — how many
    /// cells its children cover, overflow included, for scrollbar math.
    #[must_use]
    pub fn content_extent(&self, id: WidgetId) -> Option<Extent> {
        self.placed
            .iter()
            .find(|placed| placed.id == Some(id))
            .map(|placed| placed.content)
    }

    /// The deepest node's depth, or `0` when nothing was placed.
    ///
    /// The stride a stacking order needs to lift a whole overlay layer
    /// clear of the one below it.
    #[must_use]
    pub fn max_depth(&self) -> u16 {
        self.placed
            .iter()
            .map(|placed| placed.depth)
            .max()
            .unwrap_or(0)
    }

    /// Number of placed nodes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.placed.len()
    }

    /// Whether nothing was placed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.placed.is_empty()
    }
}

#[cfg(test)]
#[path = "resolved.tests.rs"]
mod tests;
