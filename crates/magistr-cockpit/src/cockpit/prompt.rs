//! The prompt: a multi-line editor with history.

use unicode_segmentation::UnicodeSegmentation as _;

use crate::render::width;

/// How many sent prompts are kept for the up arrow.
const HISTORY: usize = 100;

/// What the user is typing, where the cursor is, and what was sent
/// before.
///
/// The cursor is a byte offset on a grapheme boundary. The text can
/// span lines: [`Prompt::up`] and [`Prompt::down`] move the cursor
/// across them by display column, and the movement and kill verbs stay
/// on the cursor's line. The history is walked newest-first when the
/// cursor has no line to move to; the line being typed is kept as a
/// draft and comes back below the newest entry. Editing a recalled
/// entry makes it the draft.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Prompt {
    text: String,
    cursor: usize,
    history: Vec<String>,
    browsing: Option<usize>,
    draft: String,
}

impl Prompt {
    /// An empty prompt with no history.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The text as typed.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Whether nothing is typed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// The cursor as a byte offset into [`Prompt::text`].
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Set an insertion point chosen by a renderer; keep it on a grapheme boundary.
    pub fn set_cursor(&mut self, at: usize) {
        self.cursor = self
            .text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .chain(std::iter::once(self.text.len()))
            .take_while(|i| *i <= at)
            .last()
            .unwrap_or(0);
    }

    /// Prompts sent so far, oldest first.
    #[must_use]
    pub fn history(&self) -> &[String] {
        &self.history
    }

    /// Insert `text` at the cursor (a typed character or a paste).
    pub fn insert(&mut self, text: &str) {
        self.text.insert_str(self.cursor, text);
        self.cursor += text.len();
        self.browsing = None;
    }

    /// Delete the grapheme before the cursor.
    pub fn backspace(&mut self) {
        if let Some(start) = self.previous_boundary() {
            self.text.replace_range(start..self.cursor, "");
            self.cursor = start;
            self.browsing = None;
        }
    }

    /// Delete the grapheme at the cursor.
    pub fn delete(&mut self) {
        if let Some(end) = self.next_boundary() {
            self.text.replace_range(self.cursor..end, "");
            self.browsing = None;
        }
    }

    /// Move the cursor one grapheme left.
    pub fn left(&mut self) {
        if let Some(start) = self.previous_boundary() {
            self.cursor = start;
        }
    }

    /// Move the cursor one grapheme right.
    pub fn right(&mut self) {
        if let Some(end) = self.next_boundary() {
            self.cursor = end;
        }
    }

    /// Move the cursor to the same column of the line above; `false` on
    /// the first line (the history's turn).
    pub fn up(&mut self) -> bool {
        let start = self.line_start();
        if start == 0 {
            return false;
        }
        let column = width(&self.text[start..self.cursor]);
        let above = self.text[..start - 1].rfind('\n').map_or(0, |nl| nl + 1);
        self.cursor = above + at_column(&self.text[above..start - 1], column);
        true
    }

    /// Move the cursor to the same column of the line below; `false` on
    /// the last line (the history's turn).
    pub fn down(&mut self) -> bool {
        let end = self.line_end();
        if end == self.text.len() {
            return false;
        }
        let column = width(&self.text[self.line_start()..self.cursor]);
        let below = end + 1;
        let below_end = self.text[below..]
            .find('\n')
            .map_or(self.text.len(), |nl| below + nl);
        self.cursor = below + at_column(&self.text[below..below_end], column);
        true
    }

    /// Move the cursor to the start of the cursor's line.
    pub fn home(&mut self) {
        self.cursor = self.line_start();
    }

    /// Move the cursor to the end of the cursor's line.
    pub fn end(&mut self) {
        self.cursor = self.line_end();
    }

    /// Delete from the start of the line to the cursor (ctrl-U).
    pub fn kill_to_start(&mut self) {
        let start = self.line_start();
        if start < self.cursor {
            self.text.replace_range(start..self.cursor, "");
            self.cursor = start;
            self.browsing = None;
        }
    }

    /// Delete from the cursor to the end of the line (ctrl-K).
    pub fn kill_to_end(&mut self) {
        let end = self.line_end();
        if self.cursor < end {
            self.text.replace_range(self.cursor..end, "");
            self.browsing = None;
        }
    }

    /// Delete the word before the cursor and the spaces after it
    /// (ctrl-W), staying on the cursor's line.
    pub fn kill_word(&mut self) {
        let line = self.line_start();
        let before = &self.text[line..self.cursor];
        let trimmed = before.trim_end_matches(' ');
        let start = line + trimmed.rfind(' ').map_or(0, |space| space + 1);
        if start < self.cursor {
            self.text.replace_range(start..self.cursor, "");
            self.cursor = start;
            self.browsing = None;
        }
    }

    /// Recall the previous history entry; `false` at the oldest or with
    /// no history.
    pub fn history_up(&mut self) -> bool {
        let at = match self.browsing {
            None if self.history.is_empty() => return false,
            None => {
                self.draft = std::mem::take(&mut self.text);
                self.history.len() - 1
            }
            Some(0) => return false,
            Some(at) => at - 1,
        };
        self.recall(at);
        true
    }

    /// Recall the next history entry, or the draft below the newest;
    /// `false` when not browsing.
    pub fn history_down(&mut self) -> bool {
        let Some(at) = self.browsing else {
            return false;
        };
        if at + 1 < self.history.len() {
            self.recall(at + 1);
        } else {
            self.text = std::mem::take(&mut self.draft);
            self.cursor = self.text.len();
            self.browsing = None;
        }
        true
    }

    /// Take the text to send: the prompt empties, the text joins the
    /// history (unless it repeats the newest entry) and browsing ends.
    pub fn take(&mut self) -> String {
        let text = std::mem::take(&mut self.text);
        self.cursor = 0;
        self.browsing = None;
        self.draft.clear();
        if self.history.last() != Some(&text) {
            if self.history.len() == HISTORY {
                self.history.remove(0);
            }
            self.history.push(text.clone());
        }
        text
    }

    /// Replace `start..cursor` with an accepted completion and put the
    /// cursor after it.
    pub fn complete(&mut self, start: usize, replacement: &str) {
        self.text.replace_range(start..self.cursor, replacement);
        self.cursor = start + replacement.len();
        self.browsing = None;
    }

    /// How many lines the text spans; one when empty.
    #[must_use]
    pub fn rows(&self) -> usize {
        self.text.split('\n').count()
    }

    /// Approximate soft-wrapped rows for reserving the composer's bounded height.
    /// Pixel painting measures the actual glyphs and scrolls to the caret.
    #[must_use]
    pub fn wrapped_rows(&self, columns: u16) -> usize {
        let columns = usize::from(columns.max(1));
        self.text
            .split('\n')
            .map(|line| (usize::from(width(line)) + 1).div_ceil(columns).max(1))
            .sum()
    }

    /// The byte offset where the cursor's line starts.
    #[must_use]
    pub fn line_start(&self) -> usize {
        self.text[..self.cursor].rfind('\n').map_or(0, |nl| nl + 1)
    }

    /// The prompt as up to `max_rows` rows of `columns` cells: one
    /// `(before, at, after)` triple per visible line. On the cursor's
    /// line `at` is the grapheme under the cursor (a space at the end
    /// of the line) and the line scrolls so the cursor cell stays
    /// visible; every other line comes back in `before` alone, cut to
    /// the width. With more lines than `max_rows` the window slides so
    /// the cursor's line stays shown.
    #[must_use]
    pub fn window_rows(&self, columns: u16, max_rows: usize) -> Vec<(String, String, String)> {
        if max_rows == 0 {
            return Vec::new();
        }
        let cursor_row = self.text[..self.cursor].matches('\n').count();
        let top = (cursor_row + 1).saturating_sub(max_rows);
        let mut rows = Vec::new();
        let mut offset = 0;
        for (row, line) in self.text.split('\n').enumerate() {
            let start = offset;
            offset += line.len() + 1;
            if row < top || rows.len() == max_rows {
                continue;
            }
            if row == cursor_row {
                rows.push(line_window(line, self.cursor - start, columns));
            } else {
                rows.push((clipped(line, columns), String::new(), String::new()));
            }
        }
        rows
    }

    fn recall(&mut self, at: usize) {
        self.text.clone_from(&self.history[at]);
        self.cursor = self.text.len();
        self.browsing = Some(at);
    }

    fn line_end(&self) -> usize {
        self.text[self.cursor..]
            .find('\n')
            .map_or(self.text.len(), |nl| self.cursor + nl)
    }

    fn previous_boundary(&self) -> Option<usize> {
        self.text[..self.cursor]
            .grapheme_indices(true)
            .next_back()
            .map(|(start, _)| start)
    }

    fn next_boundary(&self) -> Option<usize> {
        self.text[self.cursor..]
            .graphemes(true)
            .next()
            .map(|g| self.cursor + g.len())
    }
}

/// The cursor's line as `columns` cells show it: the text before the
/// cursor, the grapheme under it (a space at the end of the line) and
/// the text after — scrolled so the cursor cell is always visible, the
/// whole no wider than `columns`.
fn line_window(line: &str, cursor: usize, columns: u16) -> (String, String, String) {
    let under = line[cursor..].graphemes(true).next();
    let at = under.map_or_else(|| " ".to_owned(), shown);
    if columns == 0 {
        return (String::new(), String::new(), String::new());
    }
    let room = columns.saturating_sub(width(&at));
    let mut before: Vec<String> = line[..cursor].graphemes(true).map(shown).collect();
    while !before.is_empty() && before.iter().map(|g| width(g)).sum::<u16>() > room {
        before.remove(0);
    }
    let before: String = before.concat();
    let mut after = String::new();
    let mut used = width(&before).saturating_add(width(&at));
    if let Some(end) = under.map(|g| cursor + g.len()) {
        for grapheme in line[end..].graphemes(true) {
            let cell = shown(grapheme);
            if used.saturating_add(width(&cell)) > columns {
                break;
            }
            used = used.saturating_add(width(&cell));
            after.push_str(&cell);
        }
    }
    (before, at, after)
}

/// `line` cut to `columns` cells, control characters as symbols.
fn clipped(line: &str, columns: u16) -> String {
    let mut out = String::new();
    let mut used = 0u16;
    for grapheme in line.graphemes(true) {
        let cell = shown(grapheme);
        if used.saturating_add(width(&cell)) > columns {
            break;
        }
        used = used.saturating_add(width(&cell));
        out.push_str(&cell);
    }
    out
}

/// The byte offset in `line` of the cell at display `column` — the
/// line's end when it is shorter.
fn at_column(line: &str, column: u16) -> usize {
    let mut used = 0u16;
    for (offset, grapheme) in line.grapheme_indices(true) {
        if used >= column {
            return offset;
        }
        used = used.saturating_add(width(&shown(grapheme)));
    }
    line.len()
}

/// A grapheme as the line shows it: control characters as a symbol.
fn shown(grapheme: &str) -> String {
    match grapheme {
        "\n" | "\r\n" | "\r" => "⏎".to_owned(),
        "\t" => "⇥".to_owned(),
        other => other.to_owned(),
    }
}

#[cfg(test)]
#[path = "prompt.tests.rs"]
mod tests;
