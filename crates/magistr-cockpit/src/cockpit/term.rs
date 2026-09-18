//! A program's terminal, as the screen its bytes make.

use std::collections::VecDeque;
use std::fmt;

use super::progress::Progress;
use super::rail_mark::RailMark;
use super::term_cell::TermCell;
use super::term_signals::TermSignals;
use crate::render::{Attrs, Color};

/// The screen one program has painted so far — the bytes it wrote to
/// its pseudo-terminal parsed into cells (a grapheme, its colours as
/// the program named them, its attributes), a cursor, and a scrollback
/// of what has already scrolled off the top — with the reader's own
/// place in it and, per row, the tick the row arrived on.
///
/// Pure: bytes in, a screen out, and nothing else. The bytes are kept
/// beside the parser (bounded at [`Term::KEEP`], the newest kept) so the
/// term can be cloned by replaying them and compared by them — the
/// parser itself is neither `Clone` nor `PartialEq`.
///
/// What a card shows is the **pane**: a window of [`Term::PANE_ROWS`]
/// rows over everything the program has written — the scrollback and
/// the rows of the screen it has reached — ending at the tail.
/// **Following** is the default: the window ends on the row the cursor
/// is on. A reader who scrolls up anchors the window on the rows it is
/// looking at; rows keep arriving under it and [`Term::arrived`]
/// counts them — the pill's number. Scrolling back to the tail, or
/// [`Term::follow`], resumes. The parser's own scrollback offset is
/// kept wherever the window needs it, so the window's rows are always
/// on the parser's view.
///
/// The size is the pseudo-terminal's, 80×24, until the pane's own size
/// reaches the transport (`acp-terminal-pty-size-from-pane`).
pub struct Term {
    parser: vt100::Parser<TermSignals>,
    bytes: Vec<u8>,
    /// The tick the program's first bytes arrived on — the rail's
    /// origin; `None` until it has written anything.
    started: Option<u64>,
    /// One stamp per row the program has made — the scrollback's rows
    /// first, then the screen's — the tick each arrived on.
    stamps: VecDeque<u64>,
    /// How many rows the program has made: the scrollback's plus the
    /// screen's.
    total_rows: usize,
    /// Where the pane's window ends, counted from the top of the
    /// scrollback, while the reader holds it; `None` follows the tail.
    anchor: Option<usize>,
    /// The content rows the reader last saw the tail at — what
    /// [`Term::arrived`] counts from.
    seen_rows: usize,
    finished: bool,
    /// Whether the reader ended the program with the held kill
    ///  — a finish the pane and the receipt name.
    killed: bool,
    /// The rows the window holds: [`Term::PANE_ROWS`] for a card, more
    /// while the dock is raised, never past the screen's own [`Term::ROWS`].
    window: usize,
}

impl Term {
    /// The screen's columns — the pseudo-terminal's.
    pub const COLS: u16 = 80;
    /// The screen's rows — the pseudo-terminal's.
    pub const ROWS: u16 = 24;
    /// The most rows kept above the screen.
    pub const SCROLLBACK: usize = 10_000;
    /// The most bytes kept for replay — the store's cap before the
    /// screen existed, and past it a clone starts mid-stream.
    pub const KEEP: usize = 1024 * 1024;
    /// The rows a card's pane shows of the program's output: its last
    /// eight, following the tail.
    pub const PANE_ROWS: usize = 8;

    /// A term that has seen nothing.
    #[must_use]
    pub fn new() -> Self {
        Self {
            parser: vt100::Parser::new_with_callbacks(
                Self::ROWS,
                Self::COLS,
                Self::SCROLLBACK,
                TermSignals::default(),
            ),
            bytes: Vec::new(),
            started: None,
            stamps: VecDeque::from(vec![0; usize::from(Self::ROWS)]),
            total_rows: usize::from(Self::ROWS),
            anchor: None,
            seen_rows: 1,
            finished: false,
            killed: false,
            window: Self::PANE_ROWS,
        }
    }

    /// Size the window: `rows` of the program's output end where the
    /// window ends, so widening it reaches further up and the reader's
    /// anchor holds. Clamped to one row and to the screen's
    /// [`Term::ROWS`] — the parser's view is the pseudo-terminal's, and
    /// a window taller than it has nothing to read past the top.
    pub fn set_window(&mut self, rows: usize) {
        self.window = rows.clamp(1, usize::from(Self::ROWS));
        self.sync();
    }

    /// The rows the window holds.
    #[must_use]
    pub const fn window(&self) -> usize {
        self.window
    }

    /// The rows a card shows: the last [`Term::PANE_ROWS`] of the
    /// window, or the whole window when it is smaller.
    const fn card_rows(&self) -> usize {
        if self.window < Self::PANE_ROWS {
            self.window
        } else {
            Self::PANE_ROWS
        }
    }

    /// Where the card's rows start inside the window.
    const fn card_offset(&self) -> usize {
        self.window - self.card_rows()
    }

    /// Feed the program's next chunk, written on `tick`: every row the
    /// cursor crossed — from where it stood to where it rests, every
    /// row it made on the way — is stamped with the tick.
    pub fn push(&mut self, bytes: &[u8], tick: u64) {
        self.started.get_or_insert(tick);
        self.bytes.extend_from_slice(bytes);
        if self.bytes.len() > Self::KEEP {
            self.bytes.drain(..self.bytes.len() - Self::KEEP);
        }
        let from = self.cursor_row();
        let before = self.total_rows;
        self.parser.process(bytes);
        self.measure();
        let made = self.total_rows.saturating_sub(before);
        self.stamps.extend(std::iter::repeat_n(tick, made));
        let cap = Self::SCROLLBACK + usize::from(Self::ROWS);
        while self.stamps.len() > cap {
            self.stamps.pop_front();
        }
        // Rows that fell off the front of a full scrollback moved every
        // index down by as many; the cursor's absolute row is read
        // after, so the range is clamped to what is still there.
        let to = self.cursor_row();
        let from = from.min(to);
        for at in from..=to {
            if let Some(stamp) = self.stamps.get_mut(at) {
                *stamp = tick;
            }
        }
        if self.anchor.is_none() {
            self.seen_rows = self.content_rows();
        }
        self.sync();
    }

    /// The cursor's row counted from the top of the scrollback.
    fn cursor_row(&self) -> usize {
        let (row, _) = self.parser.screen().cursor_position();
        self.scrollback_rows() + usize::from(row)
    }

    /// Re-measure the total row count after a chunk: the parser only
    /// tells how far the view can scroll, so it is asked for the
    /// furthest and then put back where it was.
    fn measure(&mut self) {
        let offset = self.parser.screen().scrollback();
        self.parser.screen_mut().set_scrollback(usize::MAX);
        let scrollback = self.parser.screen().scrollback();
        self.parser.screen_mut().set_scrollback(offset);
        self.total_rows = scrollback + usize::from(Self::ROWS);
    }

    /// The rows above the screen.
    fn scrollback_rows(&self) -> usize {
        self.total_rows - usize::from(Self::ROWS)
    }

    /// The rows the program has written: the scrollback's and the
    /// screen's down to the cursor's, counted from the top — what a
    /// scrollback thumb is sized against.
    #[must_use]
    pub fn content_rows(&self) -> usize {
        self.cursor_row() + 1
    }

    /// Where the pane's window ends, exclusive, from the top of the
    /// scrollback — the anchor while held, the content's tail while
    /// following.
    fn end(&self) -> usize {
        self.anchor.unwrap_or_else(|| self.content_rows())
    }

    /// Where the pane's window starts, from the top of the scrollback.
    fn start(&self) -> usize {
        self.end().saturating_sub(self.window)
    }

    /// Put the parser's view where the window's rows are on it: scrolled
    /// back exactly as far as the window reaches into the scrollback,
    /// not at all when the window is on the screen.
    fn sync(&mut self) {
        let back = self.scrollback_rows().saturating_sub(self.start());
        self.parser.screen_mut().set_scrollback(back);
    }

    /// Row `at` from the top of the scrollback as a row of the parser's
    /// view, when the view holds it.
    fn view_row(&self, at: usize) -> Option<u16> {
        let top = self.scrollback_rows() - self.parser.screen().scrollback();
        u16::try_from(at.checked_sub(top)?)
            .ok()
            .filter(|row| *row < Self::ROWS)
    }

    /// The bytes kept for replay — everything since the newest
    /// [`Term::KEEP`] bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// How many rows the pane's window sits above the content's tail.
    #[must_use]
    pub fn offset(&self) -> usize {
        self.content_rows().saturating_sub(self.end())
    }

    /// Whether the window sits on the live tail.
    #[must_use]
    pub fn is_following(&self) -> bool {
        self.anchor.is_none()
    }

    /// Scroll the window `rows` up into what the program wrote and
    /// hold it there. Clamped to the top of the content.
    pub fn scroll_up(&mut self, rows: usize) {
        let content = self.content_rows();
        if self.anchor.is_none() {
            self.seen_rows = content;
        }
        let end = self
            .end()
            .saturating_sub(rows)
            .max(self.window.min(content));
        self.anchor = (end < content).then_some(end);
        self.sync();
    }

    /// Scroll the window `rows` back toward the tail; reaching it
    /// resumes following.
    pub fn scroll_down(&mut self, rows: usize) {
        if let Some(held) = self.anchor {
            let content = self.content_rows();
            let end = held.saturating_add(rows);
            self.anchor = (end < content).then_some(end);
            if self.anchor.is_none() {
                self.seen_rows = content;
            }
        }
        self.sync();
    }

    /// Back to the tail, following again. Returns whether the window
    /// moved.
    pub fn follow(&mut self) -> bool {
        let moved = self.anchor.take().is_some();
        self.seen_rows = self.content_rows();
        self.sync();
        moved
    }

    /// How many rows arrived since the reader stopped following — zero
    /// while following.
    #[must_use]
    pub fn arrived(&self) -> usize {
        if self.anchor.is_none() {
            0
        } else {
            self.content_rows().saturating_sub(self.seen_rows)
        }
    }

    /// The program ended: the screen holds still and the cursor stops.
    pub fn finish(&mut self) {
        self.finished = true;
    }

    /// Whether the program ended.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.finished || self.killed
    }

    /// The reader ended the program with the held kill: the screen holds still as on any end, and the pane and
    /// the receipt say it was the human, not a failure of the
    /// program's own.
    pub fn kill(&mut self) {
        self.killed = true;
    }

    /// A refused kill cannot erase a separately observed process exit.
    pub(super) fn kill_failed(&mut self) {
        self.killed = false;
    }

    /// Whether the reader killed the program.
    #[must_use]
    pub fn is_killed(&self) -> bool {
        self.killed
    }

    /// Everything the program wrote, stripped — the scrollback and the
    /// screen as rows of plain text, trailing blanks trimmed off every
    /// row and empty rows off the end, joined by newlines: what Copy
    /// puts on the clipboard. Read off the kept bytes
    /// through a parser of its own, walked row by row from the top of
    /// the scrollback, so the reader's window and the live view stay
    /// where they are.
    #[must_use]
    pub fn transcript(&self) -> String {
        let mut parser = vt100::Parser::new(Self::ROWS, Self::COLS, Self::SCROLLBACK);
        parser.process(&self.bytes);
        parser.screen_mut().set_scrollback(usize::MAX);
        let scrollback = parser.screen().scrollback();
        parser.screen_mut().set_scrollback(0);
        let (cursor, _) = parser.screen().cursor_position();
        let content = scrollback + usize::from(cursor) + 1;
        let mut rows: Vec<String> = (0..content)
            .filter_map(|at| {
                // The view holds the screen's rows above `at`: scrolled
                // back as far as `at` is above the screen, not at all
                // once it is on it.
                let back = scrollback.saturating_sub(at);
                parser.screen_mut().set_scrollback(back);
                let top = scrollback - back;
                parser
                    .screen()
                    .rows(0, Self::COLS)
                    .nth(at - top)
                    .map(|row| row.trim_end().to_owned())
            })
            .collect();
        while rows.last().is_some_and(String::is_empty) {
            rows.pop();
        }
        rows.join("\n")
    }

    /// The cursor as `(window row, column)`, while the program runs,
    /// shows it, and the window follows the tail. `None` otherwise: a
    /// finished pane is frozen, a hidden cursor is hidden, and a held
    /// window is looking at rows the cursor left.
    #[must_use]
    pub fn window_cursor(&self) -> Option<(usize, u16)> {
        let screen = self.parser.screen();
        if self.is_finished() || screen.hide_cursor() || self.anchor.is_some() {
            return None;
        }
        let (_, col) = screen.cursor_position();
        Some((self.cursor_row() - self.start(), col))
    }

    /// The cursor as `(pane row, column)` — [`Term::window_cursor`] on
    /// the card's rows, the window's last [`Term::PANE_ROWS`].
    #[must_use]
    pub fn cursor(&self) -> Option<(usize, u16)> {
        let (row, col) = self.window_cursor()?;
        Some((row.checked_sub(self.card_offset())?, col))
    }

    /// Row `row` of the window — `0..window`, top down — as cells,
    /// every column present; `None` past the content, when the program
    /// has written fewer rows than the window holds.
    #[must_use]
    pub fn window_row(&self, row: usize) -> Option<Vec<TermCell>> {
        let at = self.start().checked_add(row)?;
        if row >= self.window || at >= self.end() {
            return None;
        }
        let view = self.view_row(at)?;
        let screen = self.parser.screen();
        Some(
            (0..Self::COLS)
                .map(|col| screen.cell(view, col).map_or_else(TermCell::default, cell))
                .collect(),
        )
    }

    /// Row `row` of the card's pane — `0..PANE_ROWS`, top down: the
    /// window's last [`Term::PANE_ROWS`] rows, so a card under a raised
    /// dock still shows the tail the dock ends on.
    #[must_use]
    pub fn pane_row(&self, row: usize) -> Option<Vec<TermCell>> {
        if row >= self.card_rows() {
            return None;
        }
        self.window_row(row + self.card_offset())
    }

    /// The tick row `row` of the window arrived on, when the window
    /// holds a row there.
    #[must_use]
    pub fn window_stamp(&self, row: usize) -> Option<u64> {
        let at = self.start().checked_add(row)?;
        if row >= self.window || at >= self.end() {
            return None;
        }
        self.stamps.get(at).copied()
    }

    /// The tick row `row` of the card's pane arrived on, when the pane
    /// holds a row there.
    #[must_use]
    pub fn stamp(&self, row: usize) -> Option<u64> {
        if row >= self.card_rows() {
            return None;
        }
        self.window_stamp(row + self.card_offset())
    }

    /// The rail's mark beside row `row` of the window — its tick, break
    /// and stamp read off the arrival ticks — when the window holds a row there.
    #[must_use]
    pub fn window_mark(&self, row: usize) -> Option<RailMark> {
        let at = self.start().checked_add(row)?;
        if row >= self.window || at >= self.end() {
            return None;
        }
        let stamp = *self.stamps.get(at)?;
        let prev = at.checked_sub(1).and_then(|p| self.stamps.get(p).copied());
        Some(RailMark::of(self.started?, prev, stamp))
    }

    /// The rail's mark beside row `row` of the card's pane, when the
    /// pane holds a row there.
    #[must_use]
    pub fn mark(&self, row: usize) -> Option<RailMark> {
        if row >= self.card_rows() {
            return None;
        }
        self.window_mark(row + self.card_offset())
    }

    /// The tick the program's first bytes arrived on, once any have.
    #[must_use]
    pub const fn started(&self) -> Option<u64> {
        self.started
    }

    /// The progress the program last reported through its terminal
    /// (OSC 9;4, read by [`TermSignals`]) — the strip under the card's
    /// head. Cleared once the program ended: the card settles on its
    /// exit code and duration, not on a bar the program never took
    /// down.
    #[must_use]
    pub fn progress(&self) -> Progress {
        if self.is_finished() {
            Progress::Cleared
        } else {
            self.parser.callbacks().progress()
        }
    }

    /// The parser's view as plain text, row by row, trailing blanks
    /// trimmed — what a surface with no cell painter shows.
    #[must_use]
    pub fn text_rows(&self) -> Vec<String> {
        self.parser
            .screen()
            .rows(0, Self::COLS)
            .map(|row| row.trim_end().to_owned())
            .collect()
    }
}

impl Default for Term {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Term {
    /// The same screen again, from the kept bytes.
    fn clone(&self) -> Self {
        let mut term = Self::new();
        term.parser.process(&self.bytes);
        term.bytes.clone_from(&self.bytes);
        term.measure();
        term.stamps.clone_from(&self.stamps);
        term.started = self.started;
        term.total_rows = self.total_rows;
        term.anchor = self.anchor;
        term.seen_rows = self.seen_rows;
        term.finished = self.finished;
        term.killed = self.killed;
        term.window = self.window;
        term.sync();
        term
    }
}

impl PartialEq for Term {
    /// Equal when the same bytes were seen, the reader is at the same
    /// place, the window is the same size and the program is in the
    /// same state — the screen is a function of those.
    fn eq(&self, other: &Self) -> bool {
        self.bytes == other.bytes
            && self.anchor == other.anchor
            && self.seen_rows == other.seen_rows
            && self.finished == other.finished
            && self.killed == other.killed
            && self.window == other.window
    }
}

impl fmt::Debug for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Term")
            .field("bytes", &self.bytes.len())
            .field("rows", &self.total_rows)
            .field("offset", &self.offset())
            .field("finished", &self.finished)
            .finish()
    }
}

/// A parser cell as the cockpit's.
fn cell(cell: &vt100::Cell) -> TermCell {
    let mut attrs = Attrs::NONE;
    for (on, attr) in [
        (cell.bold(), Attrs::BOLD),
        (cell.dim(), Attrs::DIM),
        (cell.italic(), Attrs::ITALIC),
        (cell.underline(), Attrs::UNDERLINE),
        (cell.inverse(), Attrs::REVERSE),
    ] {
        if on {
            attrs |= attr;
        }
    }
    // A cell nothing was written to is a space in the program's own
    // pen; only a wide grapheme's second column is truly empty.
    let text = match cell.contents() {
        "" if !cell.is_wide_continuation() => " ".to_owned(),
        contents => contents.to_owned(),
    };
    TermCell {
        text,
        fg: color(cell.fgcolor()),
        bg: color(cell.bgcolor()),
        attrs,
    }
}

/// A parser colour as the renderer's.
fn color(color: vt100::Color) -> Color {
    match color {
        vt100::Color::Default => Color::Default,
        vt100::Color::Idx(n) => Color::Indexed(n),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

#[cfg(test)]
#[path = "term.tests.rs"]
mod tests;
