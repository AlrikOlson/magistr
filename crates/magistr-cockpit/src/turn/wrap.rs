//! From one source line to the rows it takes at a width.

use unicode_segmentation::UnicodeSegmentation;

use super::diff_kind::diff_kind;
use super::highlight::Highlight;
use super::image::StreamImage;
use super::line_kind::LineKind;
use super::row::Row;
use super::row_kind::RowKind;
use super::span::Span;
use super::voice::Voice;
use crate::layout::TextMetrics;
use crate::render::width;
use crate::theme::Icons;
use magistr_application::sessions::{ToolStatus, ToolVerb};

/// Columns of a tool row's verb column — `delete` and `search`, the
/// longest verbs, fit exactly.
const VERB_COLUMNS: usize = 6;
/// The indent an expanded tool call's detail rows sit under.
const DETAIL_INDENT: &str = "  ";

/// The rows `text` takes at `columns` as a line of `kind`, given whether
/// a fenced code block is open at its start and, for a closed line of
/// one, the block's highlighting to advance.
///
/// Prose is classified by its first characters — a heading, a fence or a
/// fenced line, a list item, else plain — and word-wrapped with its
/// inline markdown as spans (a code line is broken by columns instead,
/// so its indentation survives, and takes the block's syntect spans when
/// `highlight` is given — the still-open tail of a streaming block wraps
/// plain; a list item's continuation rows are indented under its text);
/// a tool line is one row, its title cut to the width; a note or an
/// error word-wraps. No columns, no rows; an empty line is one empty row.
#[must_use]
pub fn wrap_line(
    kind: &LineKind,
    text: &str,
    in_fence: bool,
    highlight: Option<&mut Highlight>,
    icons: &Icons,
    columns: u16,
) -> Vec<Row> {
    if columns == 0 {
        return Vec::new();
    }
    let text = text.trim_end_matches('\r');
    match kind {
        LineKind::Prose(voice) => prose(*voice, text, in_fence, highlight, icons, columns),
        LineKind::Tool {
            status,
            verb,
            duration_ms,
            expanded,
            ..
        } => tool(
            *status,
            *verb,
            *duration_ms,
            *expanded,
            text,
            icons,
            columns,
        ),
        LineKind::Check {
            name,
            passed,
            exit_code,
            duration_ms,
            expanded,
        } => {
            let head = check_head(name, *passed, *exit_code, *duration_ms, icons);
            check(&head, *passed, *expanded, text, columns)
        }
        LineKind::Edit {
            path,
            status,
            expanded,
            undone,
            ..
        } => edit(path, *status, *expanded, *undone, text, icons, columns),
        LineKind::Image(image) => picture(image, columns),
        LineKind::Event => words(RowKind::Event, text, &[], columns),
        LineKind::Note => words(RowKind::Note, text, &[], columns),
        LineKind::Error => words(RowKind::Error, text, &[], columns),
    }
}

/// An image's rows: one blank [`RowKind::Image`] row per row of its
/// footprint at `columns`, each as wide as the placement. The rows carry
/// no text — the view fills them from the image itself, in pixels or in
/// halfblocks — but they exist so the image scrolls, is measured and is
/// found again like any other block.
fn picture(image: &StreamImage, columns: u16) -> Vec<Row> {
    let (cols, rows) = image.footprint(columns);
    if cols == 0 || rows == 0 {
        return Vec::new();
    }
    (0..rows)
        .map(|_| Row::new(RowKind::Image, " ".repeat(usize::from(cols))))
        .collect()
}

/// A tool call's rows: collapsed, one row — the status glyph, the verb
/// column, the title cut to the width, the duration once measured;
/// expanded, the same row followed by the full title word-wrapped on
/// indented code rows.
fn tool(
    status: ToolStatus,
    verb: ToolVerb,
    duration_ms: Option<u64>,
    expanded: bool,
    title: &str,
    icons: &Icons,
    columns: u16,
) -> Vec<Row> {
    let mut head = format!(
        "{} {:<VERB_COLUMNS$} {title}",
        glyph(status, icons),
        verb.label()
    );
    if let Some(ms) = duration_ms {
        head.push_str(&format!(" · {}", duration(ms)));
    }
    let mut rows = vec![Row::new(RowKind::Tool(status), cut(&head, columns))];
    if !expanded {
        return rows;
    }
    let indent = width(DETAIL_INDENT);
    if indent >= columns {
        return rows;
    }
    rows.extend(
        hard(RowKind::Code, title, &[], columns - indent)
            .into_iter()
            .map(|row| Row::new(RowKind::Code, format!("{DETAIL_INDENT}{}", row.text))),
    );
    rows
}

/// A check card's head: the outcome glyph, `gate` in the verb column,
/// the name, then exit code and duration when known.
fn check_head(
    name: &str,
    passed: bool,
    exit_code: Option<i64>,
    duration_ms: Option<u64>,
    icons: &Icons,
) -> String {
    let mut head = format!("{} {:<VERB_COLUMNS$} {name}", icons.check(passed), "gate");
    if let Some(code) = exit_code {
        head.push_str(&format!(" · exit {code}"));
    }
    if let Some(ms) = duration_ms {
        head.push_str(&format!(" · {}", duration(ms)));
    }
    head
}

/// A check card's rows: collapsed, its head cut to the width; expanded,
/// the head followed by the details on indented code rows.
fn check(head: &str, passed: bool, expanded: bool, details: &str, columns: u16) -> Vec<Row> {
    let mut rows = vec![Row::new(RowKind::Check(passed), cut(head, columns))];
    if !expanded || details.is_empty() {
        return rows;
    }
    let indent = width(DETAIL_INDENT);
    if indent >= columns {
        return rows;
    }
    for line in details.lines() {
        rows.extend(
            hard(RowKind::Code, line, &[], columns - indent)
                .into_iter()
                .map(|row| Row::new(RowKind::Code, format!("{DETAIL_INDENT}{}", row.text))),
        );
    }
    rows
}

/// The label an expanded edit card's Open button wears.
pub const EDIT_OPEN_LABEL: &str = "[Open]";
/// The label an expanded edit card's Undo button wears.
pub const EDIT_UNDO_LABEL: &str = "[Undo]";

/// An edit card's rows: collapsed, one row — the status glyph, `edit`
/// in the verb column, the path, the added/removed counts, ` · undone`
/// once the journal rewound it; expanded, the same row, a button row
/// ([Open], and [Undo] while not undone), then the diff body on
/// indented rows — added and removed lines each in their own kind,
/// hunk headers dimmed as fences, context as code.
fn edit(
    path: &str,
    status: ToolStatus,
    expanded: bool,
    undone: bool,
    body: &str,
    icons: &Icons,
    columns: u16,
) -> Vec<Row> {
    let (added, removed) =
        body.lines()
            .fold((0u32, 0u32), |(a, r), line| match line.as_bytes().first() {
                Some(b'+') => (a + 1, r),
                Some(b'-') => (a, r + 1),
                _ => (a, r),
            });
    let mut head = format!(
        "{} {:<VERB_COLUMNS$} {path} · {}{added} {}{removed}",
        glyph(status, icons),
        "edit",
        icons.added,
        icons.removed
    );
    if undone {
        head.push_str(" · undone");
    }
    let mut rows = vec![Row::new(RowKind::Tool(status), cut(&head, columns))];
    if !expanded {
        return rows;
    }
    let indent = width(DETAIL_INDENT);
    if indent >= columns {
        return rows;
    }
    let buttons = if undone {
        format!("{DETAIL_INDENT}{EDIT_OPEN_LABEL}")
    } else {
        format!("{DETAIL_INDENT}{EDIT_OPEN_LABEL}  {EDIT_UNDO_LABEL}")
    };
    rows.push(Row::new(RowKind::EditButtons, cut(&buttons, columns)));
    for line in body.lines() {
        let kind = diff_kind(line);
        rows.extend(
            hard(kind, line, &[], columns - indent)
                .into_iter()
                .map(|row| Row::new(kind, format!("{DETAIL_INDENT}{}", row.text))),
        );
    }
    rows
}

/// `873ms` under a second, `1.2s` from there on.
fn duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{}.{}s", ms / 1000, (ms % 1000) / 100)
    }
}

/// Whether a prose line opens or closes a fenced code block.
pub(crate) fn opens_fence(text: &str) -> bool {
    text.trim_start().starts_with("```")
}

/// The rule a fence line draws across the stage: `── lang ────` when it
/// opens a block whose fence names a language, a plain `────` when it
/// closes one (or names none) — the code block's frame.
fn fence_rule(text: &str, closes: bool, columns: u16) -> String {
    let lang = text.trim().trim_start_matches('`').trim();
    let mut rule = if closes || lang.is_empty() {
        String::new()
    } else {
        format!("── {lang} ")
    };
    rule.extend(std::iter::repeat_n(
        '─',
        usize::from(columns.saturating_sub(width(&rule))),
    ));
    cut(&rule, columns)
}

fn prose(
    voice: Voice,
    text: &str,
    in_fence: bool,
    highlight: Option<&mut Highlight>,
    icons: &Icons,
    columns: u16,
) -> Vec<Row> {
    if opens_fence(text) {
        return vec![Row::new(
            RowKind::Fence,
            fence_rule(text, in_fence, columns),
        )];
    }
    if in_fence {
        let spans = highlight.map_or_else(Vec::new, |block| block.line(text));
        return hard(RowKind::Code, text, &spans, columns);
    }
    let (parsed, marker, _) = super::document::parse_line(text, voice, icons);
    if let Some(marker) = marker {
        if width(&marker) >= columns {
            let (text, spans) = super::inline::parse_inline(text);
            return words(RowKind::Prose(voice), &text, &spans, columns);
        }
        let indent = width(&marker);
        let pad = " ".repeat(usize::from(indent));
        return words(parsed.kind, &parsed.text, &parsed.spans, columns - indent)
            .into_iter()
            .enumerate()
            .map(|(index, row)| {
                let lead = if index == 0 {
                    marker.as_str()
                } else {
                    pad.as_str()
                };
                let spans = row.spans.iter().map(|s| s.shifted(lead.len())).collect();
                Row::styled(row.kind, format!("{lead}{}", row.text), spans)
            })
            .collect();
    }
    words(parsed.kind, &parsed.text, &parsed.spans, columns)
}

/// The unwrapped paragraph shared by proportional wrapping. Aligned markdown
/// stays on the ordinary path without first drawing an artificial wide rule.
pub(super) fn paragraph(kind: &LineKind, text: &str, icons: &Icons) -> Option<Row> {
    let text = text.trim_end_matches('\r');
    match kind {
        LineKind::Prose(voice) => {
            if opens_fence(text) || list_item(text, icons).is_some() {
                return None;
            }
            Some(super::document::parse_line(text, *voice, icons).0)
        }
        LineKind::Note => Some(Row::new(RowKind::Note, text)),
        LineKind::Error => Some(Row::new(RowKind::Error, text)),
        _ => None,
    }
}

/// `# Title` through `###### Title` → `Title`.
pub(super) fn heading(text: &str) -> Option<&str> {
    let hashes = text.len() - text.trim_start_matches('#').len();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &text[hashes..];
    if rest.is_empty() {
        Some("")
    } else {
        rest.strip_prefix(' ').map(str::trim)
    }
}

/// A list item's marker (as shown) and its text.
pub(super) fn list_item<'a>(text: &'a str, icons: &Icons) -> Option<(String, &'a str)> {
    let trimmed = text.trim_start();
    if let Some(rest) = trimmed.strip_prefix("> ") {
        return Some(("│ ".into(), rest));
    }
    for marker in ["- ", "* ", "+ "] {
        if let Some(rest) = trimmed.strip_prefix(marker) {
            return Some((format!("{} ", icons.bullet), rest));
        }
    }
    let digits = trimmed.len()
        - trimmed
            .trim_start_matches(|c: char| c.is_ascii_digit())
            .len();
    if digits == 0 {
        return None;
    }
    let rest = trimmed[digits..].strip_prefix(". ")?;
    Some((format!("{}. ", &trimmed[..digits]), rest))
}

/// Word-wrapped rows of `kind`, each carrying its window of the line's
/// `spans`; an empty text is one empty row.
///
/// Every wrapped row is an exact substring of the measured text, so a
/// row's byte range — and with it each span's window — falls out of
/// address arithmetic on the borrowed slices.
fn words(kind: RowKind, text: &str, spans: &[Span], columns: u16) -> Vec<Row> {
    if text.is_empty() {
        return vec![Row::new(kind, "")];
    }
    let metrics = TextMetrics::new(text);
    let base = metrics.text().as_ptr() as usize;
    metrics
        .wrap(columns)
        .into_iter()
        .map(|row| {
            let start = row.as_ptr() as usize - base;
            let end = start + row.len();
            let window = spans.iter().filter_map(|s| s.window(start, end)).collect();
            Row::styled(kind, row, window)
        })
        .collect()
}

/// Rows of `kind` broken every `columns` columns, grapheme by grapheme,
/// each carrying its window of the line's `spans` — the rows are
/// consecutive substrings, so a running byte offset places them.
fn hard(kind: RowKind, text: &str, spans: &[Span], columns: u16) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut row = String::new();
    let mut start = 0;
    let mut used = 0u16;
    let window = |row: &mut String, start: usize| {
        let end = start + row.len();
        let local = spans.iter().filter_map(|s| s.window(start, end)).collect();
        Row::styled(kind, std::mem::take(row), local)
    };
    for grapheme in text.graphemes(true) {
        let w = width(grapheme);
        if used + w > columns && !row.is_empty() {
            let len = row.len();
            rows.push(window(&mut row, start));
            start += len;
            used = 0;
        }
        row.push_str(grapheme);
        used = used.saturating_add(w);
    }
    rows.push(window(&mut row, start));
    rows
}

/// `text` cut to `columns`, an ellipsis marking the cut.
fn cut(text: &str, columns: u16) -> String {
    if width(text) <= columns {
        return text.to_owned();
    }
    let keep = columns.saturating_sub(1);
    let mut out = String::new();
    let mut used = 0u16;
    for grapheme in text.graphemes(true) {
        let w = width(grapheme);
        if used + w > keep {
            break;
        }
        out.push_str(grapheme);
        used += w;
    }
    out.push('…');
    out
}

fn glyph(status: ToolStatus, icons: &Icons) -> &'static str {
    match status {
        ToolStatus::Pending => icons.pending,
        ToolStatus::InProgress => icons.running,
        ToolStatus::Completed => icons.done,
        ToolStatus::Failed => icons.failed,
    }
}

#[cfg(test)]
#[path = "wrap.tests.rs"]
mod tests;
