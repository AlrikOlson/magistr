//! Proportional paragraph wrapping with the renderer's own embedded font metrics.
use super::highlight::Highlight;
use super::wrap::{list_item, paragraph, wrap_line};
use super::{LineKind, Row};
use crate::layout::TextMetrics;
use crate::pixel::{CellSize, Typography};
use crate::theme::Icons;

/// One source line's wrap policy; aligned records keep their monospaced columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelWrap {
    pub width: u32,
    pub cell: CellSize,
    pub inline_user: bool,
}

impl PixelWrap {
    /// Select physical or terminal-cell wrapping without duplicating the line classifier.
    #[must_use]
    pub fn wrap(
        policy: Option<Self>,
        kind: &LineKind,
        text: &str,
        in_fence: bool,
        highlight: Option<&mut Highlight>,
        icons: &Icons,
        columns: u16,
    ) -> Vec<Row> {
        match policy {
            Some(policy) => policy.rows(kind, text, in_fence, highlight, icons, columns),
            None => wrap_line(kind, text, in_fence, highlight, icons, columns),
        }
    }

    #[must_use]
    pub fn rows(
        self,
        kind: &LineKind,
        text: &str,
        in_fence: bool,
        highlight: Option<&mut Highlight>,
        icons: &Icons,
        columns: u16,
    ) -> Vec<Row> {
        if in_fence || !matches!(kind, LineKind::Prose(_) | LineKind::Note | LineKind::Error) {
            return wrap_line(kind, text, in_fence, highlight, icons, columns);
        }
        // The shared markdown classifier identifies aligned records directly;
        // classification must never lay out a fence at an artificial wide size.
        let list = matches!(kind, LineKind::Prose(_))
            .then(|| list_item(text, icons))
            .flatten();
        let Some(mut row) = paragraph(kind, list.as_ref().map_or(text, |(_, rest)| rest), icons)
        else {
            return wrap_line(kind, text, false, None, icons, columns);
        };
        let mut face = Typography::default();
        face.set_cell(self.cell);
        let indent = list.as_ref().map_or(0, |(marker, _)| {
            let width = if marker.trim() == icons.bullet {
                f32::from(self.cell.width) + face.advance(" ", crate::layout::TextRole::Prose)
            } else {
                face.advance(marker, crate::layout::TextRole::Prose)
            };
            width.ceil() as u16
        });
        if list.is_some() {
            row.kind = super::RowKind::Bullet;
        }
        let role = if list.is_some() {
            crate::layout::TextRole::Prose
        } else {
            row.kind.text_role()
        };
        let width = self.width as f32
            - f32::from(indent)
            - if self.inline_user && matches!(kind, LineKind::Prose(super::Voice::User)) {
                face.advance("You · ", crate::layout::TextRole::Label)
            } else {
                0.0
            };
        std::iter::once(row)
            .flat_map(|row| {
                if row.text.is_empty() {
                    let mut row = row;
                    row.indent_px = indent;
                    row.marker = list.as_ref().map(|(marker, _)| marker.clone());
                    return vec![row];
                }
                let metrics = TextMetrics::new(&row.text);
                let mut offset = 0;
                metrics
                    .wrap_by(width.max(1.0), &mut |run| face.advance(run, role))
                    .into_iter()
                    .enumerate()
                    .map(|(index, run)| {
                        let start = offset + row.text[offset..].find(run).unwrap_or(0);
                        let end = start + run.len();
                        offset = end;
                        let mut next = Row::styled(
                            row.kind,
                            run,
                            row.spans
                                .iter()
                                .filter_map(|span| span.window(start, end))
                                .collect(),
                        );
                        next.indent_px = indent;
                        next.marker = list
                            .as_ref()
                            .filter(|_| index == 0)
                            .map(|(marker, _)| marker.clone());
                        next
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }
}
