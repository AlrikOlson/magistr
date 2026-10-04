//! Authored text, parsed blocks and source anchors change together. Conversation and
//! selected documents share inline parsing, block classification and code highlighting.
use super::{Highlight, Row, RowKind, Span, SpanStyle, Voice, parse_inline};
use crate::layout::{TextMetrics, TextRole};
use crate::theme::{Icons, ThemeMode};
use unicode_segmentation::UnicodeSegmentation;

/// A byte position in an original source line, independent of wrapping.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DocumentAnchor {
    pub line: usize,
    pub byte: usize,
}

/// One source block; tables keep labeled cells so narrow readers lose no content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentBlock {
    pub anchor: DocumentAnchor,
    pub row: Row,
    pub heading: u8,
    pub marker: Option<String>,
    pub cells: Option<Vec<(String, String)>>,
    pub table_head: bool,
    original: String,
    origins: Vec<usize>,
    parsed_cells: Vec<(Row, Row)>,
}

/// A measured row and the source position that starts it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentRow {
    pub row: Row,
    pub anchor: DocumentAnchor,
    pub heading: u8,
}

/// Transient reading content. Original bytes remain available for copying.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextDocument {
    source: String,
    blocks: Vec<DocumentBlock>,
}

impl TextDocument {
    #[must_use]
    pub fn markdown(source: impl Into<String>, mode: ThemeMode) -> Self {
        let source = source.into();
        let lines: Vec<_> = source.split('\n').collect();
        let mut blocks = Vec::new();
        let mut fence: Option<Highlight> = None;
        let mut diff = false;
        let mut index = 0;
        while index < lines.len() {
            let text = lines[index].trim_end_matches('\r');
            if fence.is_none()
                && index + 1 < lines.len()
                && let Some(headers) = table_cells(text)
                && table_separator(lines[index + 1], headers.len())
            {
                index += 2;
                let mut first = true;
                while index < lines.len() {
                    let Some(values) = table_cells(lines[index]) else {
                        break;
                    };
                    let cells: Vec<(String, String)> = (0..headers.len().max(values.len()))
                        .map(|at| {
                            (
                                headers
                                    .get(at)
                                    .cloned()
                                    .unwrap_or_else(|| format!("Column {}", at + 1)),
                                values.get(at).cloned().unwrap_or_default(),
                            )
                        })
                        .collect();
                    blocks.push(DocumentBlock {
                        anchor: DocumentAnchor {
                            line: index,
                            byte: 0,
                        },
                        row: Row::new(RowKind::Prose(Voice::Agent), ""),
                        heading: 0,
                        marker: None,
                        parsed_cells: cells
                            .iter()
                            .map(|(label, value)| {
                                let (text, spans) = parse_inline(label);
                                let label = Row::styled(RowKind::Prose(Voice::Agent), text, spans);
                                let (text, spans) = parse_inline(value);
                                (
                                    label,
                                    Row::styled(RowKind::Prose(Voice::Agent), text, spans),
                                )
                            })
                            .collect(),
                        cells: Some(cells),
                        table_head: first,
                        original: lines[index].to_owned(),
                        origins: Vec::new(),
                    });
                    first = false;
                    index += 1;
                }
                // A header without data is still authored content.
                if first {
                    blocks.push(DocumentBlock {
                        anchor: DocumentAnchor {
                            line: index.saturating_sub(2),
                            byte: 0,
                        },
                        row: Row::new(RowKind::Heading, headers.join(" | ")),
                        heading: 0,
                        marker: None,
                        parsed_cells: Vec::new(),
                        cells: None,
                        table_head: false,
                        original: text.to_owned(),
                        origins: Vec::new(),
                    });
                }
                continue;
            }
            let (row, marker, heading) = if super::wrap::opens_fence(text) {
                let closed = fence.take().is_some();
                if !closed {
                    diff = text.trim_start().trim_start_matches('`').trim() == "diff";
                    fence = Some(Highlight::for_fence(text, mode));
                }
                (
                    Row::new(
                        RowKind::Fence,
                        if closed {
                            String::new()
                        } else {
                            text.trim_start().trim_start_matches('`').trim().to_owned()
                        },
                    ),
                    None,
                    0,
                )
            } else if let Some(highlight) = &mut fence {
                (
                    Row::styled(
                        if diff {
                            super::diff_kind(text)
                        } else {
                            RowKind::Code
                        },
                        text,
                        if diff {
                            Vec::new()
                        } else {
                            highlight.line(text)
                        },
                    ),
                    None,
                    0,
                )
            } else {
                parse_line(text, Voice::Agent, &Icons::UNICODE)
            };
            blocks.push(DocumentBlock {
                anchor: DocumentAnchor {
                    line: index,
                    byte: 0,
                },
                origins: source_origins(text, &row, marker.as_deref()),
                row,
                marker,
                heading,
                parsed_cells: Vec::new(),
                cells: None,
                table_head: false,
                original: text.to_owned(),
            });
            index += 1;
        }
        Self { source, blocks }
    }

    #[must_use]
    pub fn code(source: impl Into<String>, language: &str, mode: ThemeMode) -> Self {
        let source = source.into();
        let mut highlight = Highlight::for_fence(&format!("```{language}"), mode);
        let blocks = source
            .split('\n')
            .enumerate()
            .map(|(line, text)| DocumentBlock {
                anchor: DocumentAnchor { line, byte: 0 },
                origins: (0..=text.len()).collect(),
                row: Row::styled(
                    if language == "diff" {
                        super::diff_kind(text)
                    } else {
                        RowKind::Code
                    },
                    text,
                    if language == "diff" {
                        Vec::new()
                    } else {
                        highlight.line(text)
                    },
                ),
                heading: 0,
                marker: None,
                parsed_cells: Vec::new(),
                cells: None,
                table_head: false,
                original: text.to_owned(),
            })
            .collect();
        Self { source, blocks }
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub fn blocks(&self) -> &[DocumentBlock] {
        &self.blocks
    }

    /// Surfaces supply their own ruler; the shared owner keeps content and anchors.
    pub fn rows(
        &self,
        width: f32,
        measure: &mut impl FnMut(&str, TextRole) -> f32,
    ) -> Vec<DocumentRow> {
        let width = width.max(1.0);
        let mut out = Vec::new();
        let mut table_widths = Vec::new();
        for (index, block) in self.blocks.iter().enumerate() {
            if block.table_head {
                table_widths.clear();
                for row in self.blocks[index..]
                    .iter()
                    .take_while(|row| row.cells.is_some())
                {
                    for (column, (header, value)) in row.parsed_cells.iter().enumerate() {
                        if table_widths.len() <= column {
                            table_widths.push(0.0_f32);
                        }
                        table_widths[column] = table_widths[column]
                            .max(measure(&header.text, TextRole::Data))
                            .max(measure(&value.text, TextRole::Data));
                    }
                }
            }
            if let Some(cells) = &block.cells {
                table_rows(block, cells, &table_widths, width, measure, &mut out);
            } else {
                append_rows(
                    block,
                    &block.row,
                    block.marker.as_deref(),
                    width,
                    measure,
                    &mut out,
                );
            }
        }
        out
    }

    #[must_use]
    pub fn row_of(rows: &[DocumentRow], anchor: DocumentAnchor) -> usize {
        rows.iter()
            .rposition(|row| {
                row.anchor.line < anchor.line
                    || row.anchor.line == anchor.line && row.anchor.byte <= anchor.byte
            })
            .unwrap_or(0)
    }
}

impl DocumentBlock {
    /// Table cells are parsed with their source body and reused by every surface.
    pub fn cell_rows(&self) -> impl Iterator<Item = (&Row, &Row)> {
        self.parsed_cells
            .iter()
            .map(|(label, value)| (label, value))
    }

    /// Map a displayed byte back to the author's source, excluding formatting markers.
    #[must_use]
    pub fn source_position(&self, display_byte: usize) -> DocumentAnchor {
        DocumentAnchor {
            line: self.anchor.line,
            byte: self
                .origins
                .get(display_byte)
                .copied()
                .unwrap_or(self.anchor.byte),
        }
    }
}

fn source_origins(text: &str, row: &Row, marker: Option<&str>) -> Vec<usize> {
    if matches!(
        row.kind,
        RowKind::Code | RowKind::DiffAdd | RowKind::DiffRemove | RowKind::Conflict
    ) {
        return (0..=text.len()).collect();
    }
    let content = if row.kind == RowKind::Heading {
        super::wrap::heading(text).unwrap_or(text)
    } else if marker == Some("│ ") {
        text.strip_prefix("> ").unwrap_or(text)
    } else if marker.is_some() {
        super::wrap::list_item(text, &Icons::UNICODE).map_or(text, |(_, rest)| rest)
    } else {
        text
    };
    let start = content.as_ptr() as usize - text.as_ptr() as usize;
    super::inline::parse_inline_mapped(content)
        .2
        .into_iter()
        .map(|offset| start + offset)
        .collect()
}

/// The single line classifier also used by streaming conversation wrapping.
pub(super) fn parse_line(text: &str, voice: Voice, icons: &Icons) -> (Row, Option<String>, u8) {
    if let Some(title) = super::wrap::heading(text) {
        let (display, spans) = parse_inline(title);
        let level = (text.len() - text.trim_start_matches('#').len()) as u8;
        return (Row::styled(RowKind::Heading, display, spans), None, level);
    }
    if let Some((marker, rest)) = super::wrap::list_item(text, icons) {
        let (display, spans) = parse_inline(rest);
        return (
            Row::styled(RowKind::Bullet, display, spans),
            Some(marker),
            0,
        );
    }
    let (display, spans) = parse_inline(text);
    (Row::styled(RowKind::Prose(voice), display, spans), None, 0)
}

fn append_rows(
    block: &DocumentBlock,
    row: &Row,
    marker: Option<&str>,
    width: f32,
    measure: &mut impl FnMut(&str, TextRole) -> f32,
    out: &mut Vec<DocumentRow>,
) {
    let role = row.kind.text_role();
    let indent = marker.map_or(0.0, |marker| measure(marker, role));
    let room = (width - indent).max(1.0);
    let metrics = TextMetrics::new(&row.text);
    let runs = if matches!(
        row.kind,
        RowKind::Code | RowKind::DiffAdd | RowKind::DiffRemove | RowKind::Conflict
    ) {
        hard_runs(&row.text, room, role, measure)
    } else if row.text.is_empty() {
        vec![("", 0)]
    } else {
        let base = metrics.text().as_ptr() as usize;
        metrics
            .wrap_by(room, &mut |s| measure(s, role))
            .into_iter()
            .map(|s| (s, s.as_ptr() as usize - base))
            .collect()
    };
    for (index, (run, at)) in runs.into_iter().enumerate() {
        let lead = marker.map_or(String::new(), |marker| {
            if index == 0 {
                marker.to_owned()
            } else {
                " ".repeat(marker.chars().count())
            }
        });
        let spans = row
            .spans
            .iter()
            .filter_map(|s| s.window(at, at + run.len()))
            .map(|s| s.shifted(lead.len()))
            .collect();
        let anchor = block.source_position(at);
        out.push(DocumentRow {
            row: Row::styled(row.kind, format!("{lead}{run}"), spans),
            anchor,
            heading: block.heading,
        });
    }
}

fn hard_runs<'a>(
    text: &'a str,
    width: f32,
    role: TextRole,
    measure: &mut impl FnMut(&str, TextRole) -> f32,
) -> Vec<(&'a str, usize)> {
    let mut runs = Vec::new();
    let mut start = 0;
    for (at, g) in text.grapheme_indices(true) {
        if at > start && measure(&text[start..at + g.len()], role) > width {
            runs.push((&text[start..at], start));
            start = at;
        }
    }
    runs.push((&text[start..], start));
    runs
}

fn table_cells(text: &str) -> Option<Vec<String>> {
    let text = text.trim();
    if !text.contains('|') {
        return None;
    }
    // Escaped pipes and code-span pipes are cell content, never separators.
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut escaped = false;
    let mut code = false;
    for ch in text.trim_matches('|').chars() {
        if escaped {
            cell.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => {
                cell.push(ch);
                escaped = true;
            }
            '`' => {
                code = !code;
                cell.push(ch);
            }
            '|' if !code => {
                cells.push(cell.trim().to_owned());
                cell.clear();
            }
            _ => cell.push(ch),
        }
    }
    cells.push(cell.trim().to_owned());
    Some(cells)
}

fn table_separator(text: &str, columns: usize) -> bool {
    table_cells(text).is_some_and(|cells| {
        cells.len() == columns
            && cells.iter().all(|cell| {
                let rule = cell.trim_matches(':');
                rule.len() >= 3 && rule.chars().all(|ch| ch == '-')
            })
    })
}

fn table_rows(
    block: &DocumentBlock,
    cells: &[(String, String)],
    widths: &[f32],
    width: f32,
    measure: &mut impl FnMut(&str, TextRole) -> f32,
    out: &mut Vec<DocumentRow>,
) {
    let parsed = &block.parsed_cells;
    let space = measure(" ", TextRole::Data).max(0.01);
    let wide = widths.iter().sum::<f32>()
        + measure(" | ", TextRole::Data) * widths.len().saturating_sub(1) as f32
        <= width;
    if wide {
        for header in [true, false] {
            if header && !block.table_head {
                continue;
            }
            let mut text = String::new();
            let mut spans = Vec::new();
            for (index, pair) in parsed.iter().enumerate() {
                let cell = if header { &pair.0 } else { &pair.1 };
                let value = &cell.text;
                let style = &cell.spans;
                if !header {
                    spans.extend(style.iter().map(|s| s.shifted(text.len())));
                }
                if header {
                    spans.push(Span::new(
                        text.len(),
                        text.len() + value.len(),
                        SpanStyle::Bold,
                    ));
                }
                text.push_str(value);
                let pad = ((widths[index] - measure(value, TextRole::Data)) / space)
                    .ceil()
                    .max(0.0) as usize;
                text.push_str(&" ".repeat(pad));
                if index + 1 < cells.len() {
                    text.push_str(" | ");
                }
            }
            let row = Row::styled(RowKind::Code, text.trim_end(), spans);
            append_rows(block, &row, None, width, measure, out);
        }
    } else {
        for (label, value) in parsed {
            let prefix = format!("{}: ", label.text);
            let mut spans = vec![Span::new(0, label.text.len() + 1, SpanStyle::Bold)];
            spans.extend(value.spans.iter().map(|s| s.shifted(prefix.len())));
            append_rows(
                block,
                &Row::styled(
                    RowKind::Prose(Voice::Agent),
                    format!("{prefix}{}", value.text),
                    spans,
                ),
                None,
                width,
                measure,
                out,
            );
        }
        out.push(DocumentRow {
            row: Row::new(RowKind::Prose(Voice::Agent), ""),
            anchor: block.anchor,
            heading: 0,
        });
    }
}

#[cfg(test)]
#[path = "document.tests.rs"]
mod tests;
