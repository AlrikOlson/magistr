//! Native rich text paints shared spans and owns native word wrapping.
use crate::{Element, Lexicon, Voice};
use magistr_cockpit::turn::{Row, RowKind, SpanStyle};
use qs_gpu::color::Srgba;

pub(crate) struct RichText;
impl RichText {
    pub(crate) fn cell(label: &Row, value: &Row, lex: &Lexicon) -> Element {
        let prefix = format!("{}: ", label.text);
        let mut spans = vec![magistr_cockpit::turn::Span::new(
            0,
            label.text.len() + 1,
            SpanStyle::Bold,
        )];
        spans.extend(value.spans.iter().map(|span| span.shifted(prefix.len())));
        Self::row(
            &Row::styled(
                RowKind::Prose(magistr_cockpit::turn::Voice::Agent),
                format!("{prefix}{}", value.text),
                spans,
            ),
            lex.tx,
            lex,
        )
    }
    pub(crate) fn row(row: &Row, ink: Srgba, lex: &Lexicon) -> Element {
        if row.spans.is_empty() {
            return Element::text(Self::voice(row), Self::ink(row, ink, lex), &row.text)
                .min_width(0.0)
                .claim(crate::Component::Prose);
        }
        Self::marked(row, ink, lex, &mut |_| None)
    }
    fn voice(row: &Row) -> Voice {
        let voice = Voice::of(row.kind.text_role());
        if voice.px <= 0.0 { Voice::VALUE } else { voice }
    }
    fn ink(row: &Row, ink: Srgba, lex: &Lexicon) -> Srgba {
        match row.kind {
            RowKind::DiffAdd => lex.green,
            RowKind::DiffRemove | RowKind::Conflict => lex.red,
            _ => ink,
        }
    }
    /// Each word may carry a source marker; native layout reports its actual y position.
    pub(crate) fn marked(
        row: &Row,
        ink: Srgba,
        lex: &Lexicon,
        mark: &mut impl FnMut(usize) -> Option<crate::ElementId>,
    ) -> Element {
        let voice = Self::voice(row);
        let ink = Self::ink(row, ink, lex);
        let mut container = Element::row().wrap().min_width(0.0).stretch().no_shrink();
        let mut offset = 0;
        for word in row.text.split_inclusive(' ') {
            let end = offset + word.len();
            let mut part = Element::row().min_width(0.0).no_shrink();
            if let Some(id) = mark(offset) {
                part = part.id(id);
            }
            let mut at = offset;
            for span in row.spans.iter().filter(|s| s.end > offset && s.start < end) {
                let start = span.start.max(at).min(end);
                if start > at {
                    part = part.child(
                        Element::text(voice, ink, &row.text[at..start])
                            .min_width(0.0)
                            .no_shrink(),
                    );
                }
                let stop = span.end.min(end);
                if stop <= start {
                    continue;
                }
                let mut face = voice;
                let mut color = ink;
                match &span.style {
                    SpanStyle::Bold => face.weight = 700,
                    SpanStyle::Italic => face = face.italic(),
                    SpanStyle::Code => face.family = "mono",
                    SpanStyle::Link(_) => color = lex.mauve,
                    SpanStyle::Fg(fg) => {
                        color =
                            Lexicon::readable(lex.text_color(*fg).unwrap_or(ink), lex.mantle, 4.5)
                    }
                }
                part = part.child(
                    Element::text(face, color, &row.text[start..stop])
                        .min_width(0.0)
                        .no_shrink()
                        .claim(crate::Component::Prose),
                );
                at = stop;
            }
            if at < end {
                part = part.child(
                    Element::text(voice, ink, &row.text[at..end])
                        .min_width(0.0)
                        .no_shrink(),
                );
            }
            container = container.child(part);
            offset = end;
        }
        if row.text.is_empty() {
            container = container.height(8.0);
        }
        container
    }
}

#[cfg(test)]
#[path = "rich_text.tests.rs"]
mod tests;
