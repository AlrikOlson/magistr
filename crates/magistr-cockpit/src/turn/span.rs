//! One styled run within a row's text.

use super::span_style::SpanStyle;

/// A styled span: a byte range over the text it annotates and the style
/// laid over that range. Spans are ordered and never overlap.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    /// First byte of the run.
    pub start: usize,
    /// One past the last byte of the run.
    pub end: usize,
    /// The style laid over the run.
    pub style: SpanStyle,
}

impl Span {
    /// A span of `style` over `start..end`.
    #[must_use]
    pub const fn new(start: usize, end: usize, style: SpanStyle) -> Self {
        Self { start, end, style }
    }

    /// The part of this span inside `start..end`, re-based to it — the
    /// row's slice of a line-level span. `None` when they do not overlap.
    #[must_use]
    pub fn window(&self, start: usize, end: usize) -> Option<Self> {
        let from = self.start.max(start);
        let to = self.end.min(end);
        if from >= to {
            return None;
        }
        Some(Self::new(from - start, to - start, self.style.clone()))
    }

    /// This span moved `by` bytes to the right.
    #[must_use]
    pub fn shifted(&self, by: usize) -> Self {
        Self::new(self.start + by, self.end + by, self.style.clone())
    }
}
