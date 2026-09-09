//! The identity a widget keeps across layout, painting and hit-testing.

/// A stable, cheap identifier for one interactive widget.
///
/// Layout attaches a [`WidgetId`] to the rect it resolves for a node and
/// the hit registry hands the same id back for a mouse position. The value
/// has no meaning to the renderer: the cockpit chooses ids however it likes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WidgetId(u32);

impl WidgetId {
    /// Wrap a raw id.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// The raw id.
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

#[cfg(test)]
#[path = "widget_id.tests.rs"]
mod tests;
