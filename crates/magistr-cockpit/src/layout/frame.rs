//! The border a framed node wears.

use crate::render::Style;

/// A panel's border: rounded box-drawing corners and edges painted in one
/// style, an optional title embedded in the top border (in its own style
/// when given), and an optional meta slot right-aligned in the top border.
///
/// A framed [`Node`](super::Node) reserves one cell on every edge through
/// the layout engine itself — children are laid out inside the ring, so
/// the painter never does coordinate math the tree has not already done.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    title: Option<String>,
    meta: Option<String>,
    style: Style,
    title_style: Option<Style>,
}

impl Frame {
    /// A plain ring in `style`, no title, no meta.
    #[must_use]
    pub const fn new(style: Style) -> Self {
        Self {
            title: None,
            meta: None,
            style,
            title_style: None,
        }
    }

    /// Embed `title` in the top border, after the first corner.
    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Right-align `meta` in the top border, before the last corner.
    #[must_use]
    pub fn meta(mut self, meta: impl Into<String>) -> Self {
        self.meta = Some(meta.into());
        self
    }

    /// Paint the title in `style` instead of the ring's own.
    #[must_use]
    pub fn title_style(mut self, style: Style) -> Self {
        self.title_style = Some(style);
        self
    }

    /// The ring's style.
    #[must_use]
    pub const fn border_style(&self) -> &Style {
        &self.style
    }

    /// The title to embed, if any.
    #[must_use]
    pub fn title_text(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// The meta to right-align, if any.
    #[must_use]
    pub fn meta_text(&self) -> Option<&str> {
        self.meta.as_deref()
    }

    /// The style the title is painted in — its own when set, else the
    /// ring's.
    #[must_use]
    pub const fn heading_style(&self) -> &Style {
        match &self.title_style {
            Some(style) => style,
            None => &self.style,
        }
    }
}

#[cfg(test)]
#[path = "frame.tests.rs"]
mod tests;
