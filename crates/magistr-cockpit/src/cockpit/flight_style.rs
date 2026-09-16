//! Shared pixel spacing, surfaces and text for the Flight Recorder interface.

use super::Model;
use crate::layout::{Align, Length, Node, TextRole};
use crate::render::{Attrs, Color, Style};
use crate::theme::{Pen, ThemeMode};
use magistr_theme::ThemeColor;
use magistr_theme::math::ensure_contrast;
use unicode_segmentation::UnicodeSegmentation;

/// The finished interface's surfaces and text, shared by its panels.
#[derive(Clone, Copy, Debug)]
pub struct FlightStyle {
    pub ink: Color,
    pub muted: Color,
    pub accent: Color,
    pub good: Color,
    pub line: Color,
    pub root: Color,
    pub panel: Color,
    pub header: Color,
    pub raised: Color,
    pub selected: Color,
    pub line_height: f32,
}

impl FlightStyle {
    #[must_use]
    pub fn for_model(model: &Model) -> Self {
        let theme = model.theme();
        let pen = Pen::for_theme(&theme);
        if theme.mode() == ThemeMode::Dark {
            Self {
                ink: Color::Rgb(224, 231, 241),
                muted: Color::Rgb(158, 174, 193),
                accent: Color::Rgb(151, 191, 255),
                good: if theme.cvd().is_some() {
                    pen.gate_pass.fg
                } else {
                    Color::Rgb(127, 217, 177)
                },
                line: Color::Rgb(76, 76, 98),
                root: Color::Rgb(17, 17, 27),
                panel: Color::Rgb(30, 30, 46),
                header: Color::Rgb(39, 39, 57),
                raised: Color::Rgb(45, 45, 65),
                selected: Color::Rgb(49, 65, 92),
                line_height: f32::from(model.cell_size().height),
            }
        } else {
            Self {
                ink: pen.base.fg,
                muted: pen.note.fg,
                accent: pen.link.fg,
                good: pen.gate_pass.fg,
                line: Color::Rgb(175, 179, 193),
                root: Color::Rgb(239, 241, 245),
                panel: Color::Rgb(250, 251, 253),
                header: Color::Rgb(229, 232, 240),
                raised: Color::Rgb(235, 238, 245),
                selected: Color::Rgb(213, 225, 247),
                line_height: f32::from(model.cell_size().height),
            }
        }
    }

    #[must_use]
    pub fn style(&self, ink: Color) -> Style {
        Style::new().fg(contrast(ink, self.panel)).bg(self.panel)
    }

    #[must_use]
    pub fn label(&self, text: impl Into<String>, ink: Color) -> Node {
        Node::text(text)
            .text_role(TextRole::Label)
            .style(self.style(ink))
            .min_height(Length::Px(self.line_height))
            .min_width(Length::Px(0.0))
            .shrink(0.0)
    }

    #[must_use]
    pub fn heading(&self, text: impl Into<String>) -> Node {
        self.label(text, self.ink)
            .text_role(TextRole::Heading)
            .style(self.style(self.ink).attrs(Attrs::BOLD))
    }

    #[must_use]
    pub fn prose(&self, text: impl Into<String>) -> Node {
        Node::text(text)
            .text_role(TextRole::Prose)
            .style(self.style(self.ink))
            .min_width(Length::Px(0.0))
            .shrink(0.0)
    }

    #[must_use]
    pub fn chip(&self, text: impl Into<String>, ink: Color, background: Color) -> Node {
        self.ground(self.inset(self.label(text, ink), 10.0, 2.0), background)
    }

    #[must_use]
    pub fn ground(&self, mut node: Node, color: Color) -> Node {
        node.restyle_all(&|s| s.clone().fg(contrast(s.fg, color)).bg(color));
        node
    }

    #[must_use]
    pub fn flex() -> Node {
        Node::leaf().grow(1.0).basis(Length::Px(0.0))
    }

    #[must_use]
    pub fn hspace(width: f32) -> Node {
        Node::leaf().width(Length::Px(width)).shrink(0.0)
    }

    #[must_use]
    pub fn vspace(height: f32) -> Node {
        Node::leaf().height(Length::Px(height)).shrink(0.0)
    }

    #[must_use]
    pub fn inset(&self, content: Node, x: f32, y: f32) -> Node {
        Node::column()
            .min_width(Length::Px(0.0))
            .min_height(Length::Px(0.0))
            .style(self.style(self.ink))
            .child(Self::vspace(y))
            .child(
                Node::row()
                    .grow(1.0)
                    .min_height(Length::Px(0.0))
                    .child(Self::hspace(x))
                    .child(
                        content
                            .grow(1.0)
                            .min_width(Length::Px(0.0))
                            .min_height(Length::Px(0.0)),
                    )
                    .child(Self::hspace(x)),
            )
            .child(Self::vspace(y))
    }

    #[must_use]
    pub fn outlined(&self, content: Node, fill: Color, border: Color) -> Node {
        Node::column()
            .min_width(Length::Px(0.0))
            .min_height(Length::Px(0.0))
            .style(self.style(self.ink).bg(border))
            .child(Self::vspace(1.0))
            .child(
                Node::row()
                    .grow(1.0)
                    .min_height(Length::Px(0.0))
                    .child(Self::hspace(1.0))
                    .child(
                        content
                            .grow(1.0)
                            .min_width(Length::Px(0.0))
                            .min_height(Length::Px(0.0))
                            .style(self.style(self.ink).bg(fill)),
                    )
                    .child(Self::hspace(1.0)),
            )
            .child(Self::vspace(1.0))
    }

    #[must_use]
    pub fn section_heading(&self, title: impl Into<String>, aside: Node) -> Node {
        self.ground(
            self.inset(
                Node::row()
                    .align_items(Align::Center)
                    .child(self.heading(title))
                    .child(Self::flex())
                    .child(aside),
                16.0,
                0.0,
            ),
            self.header,
        )
        .height(Length::Px((self.line_height + 14.0).max(34.0)))
        .shrink(0.0)
    }

    /// A label shortened at a grapheme boundary to fit its measured pixel width.
    #[must_use]
    pub fn fit_label(text: &str, width: f32, typography: &crate::pixel::Typography) -> String {
        if typography.advance(text, TextRole::Label) <= width {
            return text.to_owned();
        }
        let mut result = String::new();
        let available = (width - typography.advance("…", TextRole::Label)).max(0.0);
        for grapheme in text.graphemes(true) {
            if typography.advance(&result, TextRole::Label)
                + typography.advance(grapheme, TextRole::Label)
                > available
            {
                break;
            }
            result.push_str(grapheme);
        }
        result.push('…');
        result
    }

    /// A lane's label ink, held legible against the question card.
    #[must_use]
    pub fn lane(&self, lane: u8) -> Color {
        let [r, g, b] = crate::pixel::BranchGraph::lane_rgb(lane);
        contrast(Color::Rgb(r, g, b), self.raised)
    }
}

fn contrast(fg: Color, bg: Color) -> Color {
    let (Color::Rgb(r, g, b), Color::Rgb(br, bg, bb)) = (fg, bg) else {
        return fg;
    };
    let adjusted = ensure_contrast(ThemeColor::new(r, g, b), ThemeColor::new(br, bg, bb), 4.5);
    Color::Rgb(adjusted.r, adjusted.g, adjusted.b)
}

#[cfg(test)]
#[path = "flight_style.tests.rs"]
mod tests;
