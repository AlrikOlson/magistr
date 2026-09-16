//! A line of text with vocabulary glyphs in it, built once for both
//! kinds of terminal.

use super::model::Model;
use crate::layout::{Length, Mark, Node};
use crate::pixel::Icon;
use crate::render::{Style, width};
use crate::theme::Pen;

/// A line the view builds glyph by glyph. On a plain terminal each
/// glyph is the set's character. On a graphics terminal each icon has its
/// own layout slot, with proportional text runs beside it. Placeholder
/// columns are only a construction detail and never position pixel text.
#[derive(Clone, Debug)]
pub struct Marked<'a> {
    pen: &'a Pen,
    pixels: bool,
    text: String,
    marks: Vec<Mark>,
}

impl<'a> Marked<'a> {
    /// An empty line for `model`'s terminal, spelled with `pen`.
    #[must_use]
    pub fn new(model: &Model, pen: &'a Pen) -> Self {
        Self {
            pen,
            pixels: model.has_pixels(),
            text: String::new(),
            marks: Vec::new(),
        }
    }

    /// Append `icon`: its glyph, or a blank cell that carries the mark.
    #[must_use]
    pub fn icon(mut self, icon: Icon) -> Self {
        if self.pixels {
            let col = width(&self.text);
            self.text.push(' ');
            self.marks.push(Mark::Icon {
                icon,
                col,
                cols: 1,
                ink: self.pen.ink(icon),
            });
        } else {
            self.text.push_str(self.pen.icons.of(icon));
        }
        self
    }

    /// Append plain text.
    #[must_use]
    pub fn text(mut self, text: &str) -> Self {
        self.text.push_str(text);
        self
    }

    /// The line as a text node carrying its marks.
    #[must_use]
    pub fn node(self) -> Node {
        if !self.pixels || self.marks.is_empty() {
            return Node::text(self.text);
        }
        use unicode_segmentation::UnicodeSegmentation as _;
        let mut node = Node::row().height(Length::Cells(1));
        let mut run = String::new();
        let mut col = 0;
        for grapheme in self.text.graphemes(true) {
            let mark = self
                .marks
                .iter()
                .find(|mark| matches!(mark, Mark::Icon { col: at, .. } if *at == col));
            if let Some(Mark::Icon {
                icon, cols, ink, ..
            }) = mark
            {
                if !run.is_empty() {
                    node = node.child(Node::text(std::mem::take(&mut run)).shrink(0.0));
                }
                node = node.child(
                    Node::leaf()
                        .width(Length::Cells(*cols))
                        .height(Length::Cells(1))
                        .shrink(0.0)
                        .mark(Mark::Icon {
                            icon: *icon,
                            col: 0,
                            cols: *cols,
                            ink: *ink,
                        }),
                );
            } else {
                run.push_str(grapheme);
            }
            col += width(grapheme);
        }
        if !run.is_empty() {
            node = node.child(Node::text(run).shrink(0.0));
        }
        node
    }

    /// The line as a one-row segment in `style` that never gives way —
    /// the shape every chrome row is made of.
    #[must_use]
    pub fn segment(self, style: Style) -> Node {
        self.node()
            .style(style)
            .height(Length::Cells(1))
            .shrink(0.0)
    }
}

#[cfg(test)]
#[path = "marked.tests.rs"]
mod tests;
