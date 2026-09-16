//! Measuring and wrapping text in cell columns.

use unicode_segmentation::UnicodeSegmentation;

use crate::render::width;

/// Text as the layout engine sees it: intrinsic widths and the number of
/// rows it needs at a given width.
///
/// Wrapping is greedy by word (single spaces separate words, `\n` forces a
/// line, a line's leading spaces stay on its first row); a word wider than the available columns breaks at grapheme
/// boundaries; a lone grapheme wider than the columns still takes a line
/// rather than vanishing. Widths are measured per grapheme cluster through
/// [`width`], so CJK and emoji count their two columns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextMetrics {
    text: String,
    max_content: u16,
    min_content: u16,
    role: super::TextRole,
    cursor: Option<usize>,
}

impl TextMetrics {
    /// Metrics over `text`. The intrinsic widths are measured once here:
    /// the layout engine asks for them several times per pass.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let max_content = text.lines().map(width).max().unwrap_or(0);
        let min_content = text
            .lines()
            .flat_map(|line| line.split(' '))
            .map(width)
            .max()
            .unwrap_or(0);
        Self {
            text,
            max_content,
            min_content,
            role: super::TextRole::Label,
            cursor: None,
        }
    }

    /// Insertion position for a pixel editor, as a byte boundary.
    #[must_use]
    pub const fn cursor(&self) -> Option<usize> {
        self.cursor
    }

    /// Mark this leaf as an editor. Invalid byte offsets are clamped backward.
    pub fn set_cursor(&mut self, cursor: usize) {
        let mut at = cursor.min(self.text.len());
        while !self.text.is_char_boundary(at) {
            at -= 1;
        }
        self.cursor = Some(at);
        self.role = super::TextRole::Steer;
    }

    /// The semantic voice used by pixel measurement and painting.
    #[must_use]
    pub const fn role(&self) -> super::TextRole {
        self.role
    }

    /// Select a voice without changing the cell fallback's metrics.
    pub fn set_role(&mut self, role: super::TextRole) {
        self.role = role;
    }

    /// The text being measured.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Columns needed to show every line unwrapped: the widest line.
    #[must_use]
    pub fn max_content_width(&self) -> u16 {
        self.max_content
    }

    /// The narrowest width that does not break a word: the widest word.
    #[must_use]
    pub fn min_content_width(&self) -> u16 {
        self.min_content
    }

    /// Rows the text needs when wrapped to `columns` (zero when it has no
    /// columns to wrap into).
    #[must_use]
    pub fn wrapped_height(&self, columns: u16) -> u16 {
        u16::try_from(self.wrap(columns).len()).unwrap_or(u16::MAX)
    }

    /// The text wrapped to `columns`, one entry per row, borrowing from the
    /// original text (an empty source line yields one empty row).
    #[must_use]
    pub fn wrap(&self, columns: u16) -> Vec<&str> {
        self.wrap_by(f32::from(columns), &mut |run| f32::from(width(run)))
    }

    /// The same greedy wrap against an arbitrary ruler: `limit` units
    /// per row, `measure` answering how many units a run of the text is
    /// worth.
    ///
    /// The one wrapper, not two. A proportional face on the window
    /// breaks a paragraph in different places from a monospaced one on
    /// a terminal, but it breaks it by the *same rule* — and, more to
    /// the point, the painter wraps through this too, so the rows drawn
    /// are the rows the layout measured rather than two greedy loops
    /// that happen to agree.
    ///
    /// `measure` is asked about whole words, about the single space
    /// that joins them and about individual graphemes when a word has
    /// to be broken, so a shaper's advances are summed per run: kerning
    /// *across* a join is not counted, which costs a fraction of a
    /// pixel a row and buys a wrap that can be computed without
    /// re-shaping the whole paragraph per candidate break.
    #[must_use]
    pub fn wrap_by<'a>(&'a self, limit: f32, measure: &mut dyn FnMut(&str) -> f32) -> Vec<&'a str> {
        let mut lines = Vec::new();
        if limit <= 0.0 {
            return lines;
        }
        let space = measure(" ");
        for paragraph in self.text.lines() {
            wrap_paragraph(paragraph, limit, space, measure, &mut |line| {
                lines.push(line);
            });
        }
        lines
    }
}

/// A run of the paragraph accumulating on the current row.
struct Row {
    start: usize,
    end: usize,
    units: f32,
}

fn wrap_paragraph<'a>(
    paragraph: &'a str,
    limit: f32,
    space: f32,
    measure: &mut dyn FnMut(&str) -> f32,
    emit: &mut impl FnMut(&'a str),
) {
    // A paragraph's leading spaces are content — an indented code line, a
    // list item's continuation — and open the first row; the first word
    // then joins them through the separator the loop counts.
    let lead = paragraph.len() - paragraph.trim_start_matches(' ').len();
    let mut row: Option<Row> = (lead > 0).then(|| Row {
        start: 0,
        end: lead,
        // Less one separator, which the loop adds back when the first
        // word joins these spaces.
        units: measure(&paragraph[..lead]) - space,
    });
    let mut emitted = false;
    let mut offset = 0usize;
    for word in paragraph.split(' ') {
        let start = offset;
        offset += word.len() + 1;
        if word.is_empty() {
            continue;
        }
        let word_units = measure(word);
        if word_units > limit {
            if let Some(open) = row.take() {
                emit(&paragraph[open.start..open.end]);
                emitted = true;
            }
            row = break_word(paragraph, start, word, limit, measure, emit, &mut emitted);
            continue;
        }
        match row.as_mut() {
            Some(open) if open.units + space + word_units <= limit => {
                open.end = start + word.len();
                open.units += space + word_units;
            }
            Some(open) => {
                emit(&paragraph[open.start..open.end]);
                emitted = true;
                *open = Row {
                    start,
                    end: start + word.len(),
                    units: word_units,
                };
            }
            None => {
                row = Some(Row {
                    start,
                    end: start + word.len(),
                    units: word_units,
                });
            }
        }
    }
    if let Some(open) = row {
        emit(&paragraph[open.start..open.end]);
        emitted = true;
    }
    if !emitted {
        emit(&paragraph[0..0]);
    }
}

/// Split a word wider than `limit` at grapheme boundaries, emitting every
/// full row and returning the partial last row so following words can join
/// it.
fn break_word<'a>(
    paragraph: &'a str,
    word_start: usize,
    word: &str,
    limit: f32,
    measure: &mut dyn FnMut(&str) -> f32,
    emit: &mut impl FnMut(&'a str),
    emitted: &mut bool,
) -> Option<Row> {
    let mut row: Option<Row> = None;
    for (index, grapheme) in word.grapheme_indices(true) {
        let grapheme_units = measure(grapheme);
        let start = word_start + index;
        let end = start + grapheme.len();
        match row.as_mut() {
            Some(open) if open.units + grapheme_units <= limit => {
                open.end = end;
                open.units += grapheme_units;
            }
            Some(open) => {
                emit(&paragraph[open.start..open.end]);
                *emitted = true;
                *open = Row {
                    start,
                    end,
                    units: grapheme_units,
                };
            }
            None => {
                row = Some(Row {
                    start,
                    end,
                    units: grapheme_units,
                });
            }
        }
    }
    row
}

#[cfg(test)]
#[path = "text.tests.rs"]
mod tests;
