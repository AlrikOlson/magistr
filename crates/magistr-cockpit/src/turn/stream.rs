//! The turn's lines and rows, kept current as the agent streams.

use super::ConversationLayout;
use super::accent::Accent;
use super::anchor::Anchor;
use super::highlight::Highlight;
use super::image::{MAX_IMAGE_ROWS, StreamImage};
use super::line::Line;
use super::line_kind::LineKind;
use super::pixel_wrap::PixelWrap;
use super::receipt::Receipt;
use super::row::Row;
use super::row_kind::RowKind;
use super::voice::Voice;
use super::wrap::opens_fence;
use crate::agent::{ToolRecord, ToolStatus, ToolVerb};
use crate::pixel::CellSize;
use crate::theme::{Icons, ThemeMode};

/// A turn: source lines in arrival order and the rows they wrap to at
/// the current width.
///
/// The last line may be *open* — a prose line still receiving deltas of
/// its voice. A delta appends to it and re-wraps only its rows; a
/// newline closes it and carries the fence state forward — a closed line
/// of a fenced block runs through syntect exactly once here, while the
/// open tail renders plain. A delta in another voice, a tool call, a
/// note or an error closes it too. A tool call is one line and one row,
/// found again by id and replaced in place on an update, so no later row
/// moves. Changing the width — or the theme mode — re-wraps every line
/// once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Turn {
    lines: Vec<Line>,
    rows: Vec<Row>,
    width: u16,
    pixel_wrap: Option<PixelWrap>,
    conversation: ConversationLayout,
    conversation_compact: bool,
    /// Columns a MONO line is cut or broken to, when the surface holds
    /// its mono content to a narrower measure than its prose — see
    /// [`Turn::set_mono_width`]. `None` means one width for everything,
    /// which is what a cell surface always wants.
    mono: Option<u16>,
    /// The fenced code block open at the start of the last line, with
    /// its highlighting advanced through every closed line.
    fence: Option<Highlight>,
    /// Whether the last line is a prose line still receiving deltas.
    open: bool,
    /// The theme mode the code highlighting draws for.
    mode: ThemeMode,
    /// The glyph vocabulary the tool, check and edit heads are drawn in.
    icons: &'static Icons,
    /// The terminal cell every image block is measured against.
    cell: CellSize,
    /// The tallest an image block may stand, in rows — the terminal's
    /// [`MAX_IMAGE_ROWS`] until the surface says otherwise.
    row_cap: u16,
    /// The receipt each completed call earned, by call id — read only
    /// by the native card's head and trailer, so it lives beside the
    /// lines rather than inside [`LineKind`] and no row ever re-wraps
    /// for it.
    receipts: std::collections::BTreeMap<String, Receipt>,
    /// How many times rows before the tail have moved — bumped by every
    /// re-wrap, whole or of one line — so a window held to a row can
    /// tell that its row may have moved (see [`Turn::anchor_at`]).
    layout: u64,
    /// Rows the last layout change rebuilt: every row for a whole
    /// re-wrap, one card's for a toggle — the cost a test bounds.
    rebuilt: usize,
}

impl Turn {
    /// The rows an open card's terminal pane takes: the window
    /// [`Term`](crate::cockpit::Term) shows of the program's output.
    pub const PANE_ROWS: usize = crate::cockpit::Term::PANE_ROWS;

    /// An empty turn wrapping at `width` columns, highlighting for the
    /// dark theme until [`Turn::set_mode`] says otherwise.
    #[must_use]
    pub const fn new(width: u16) -> Self {
        Self {
            lines: Vec::new(),
            rows: Vec::new(),
            width,
            pixel_wrap: None,
            conversation: ConversationLayout::new(),
            conversation_compact: false,
            mono: None,
            fence: None,
            open: false,
            mode: ThemeMode::Dark,
            icons: &Icons::UNICODE,
            cell: CellSize::DEFAULT,
            row_cap: MAX_IMAGE_ROWS,
            receipts: std::collections::BTreeMap::new(),
            layout: 0,
            rebuilt: 0,
        }
    }

    /// The smaller conversation uses the same source rows with shorter work summaries.
    pub fn set_conversation_compact(&mut self, compact: bool) {
        self.conversation_compact = compact;
    }

    /// Change the paragraph measure without changing source identities or terminal alignment.
    pub fn set_pixel_wrap(&mut self, width: Option<u32>, cell: CellSize) {
        let policy = width.map(|width| PixelWrap {
            width,
            cell,
            inline_user: self.conversation_compact,
        });
        if self.pixel_wrap != policy {
            self.pixel_wrap = policy;
            self.rewrap(self.width);
        }
        let next = policy.map_or_else(ConversationLayout::new, |policy| {
            ConversationLayout::of(self, policy.width as f32, cell, self.conversation_compact)
        });
        if self.conversation != next {
            self.conversation = next;
            self.layout = self.layout.wrapping_add(1);
        }
    }

    /// Cached physical positions, refreshed with the pixel wrap policy after each model fold.
    #[must_use]
    pub const fn conversation(&self) -> &ConversationLayout {
        &self.conversation
    }

    /// How many times rows before the tail have moved: a re-wrap of the
    /// whole turn or of one line. A window pinned to a row reads it
    /// across a fold — unchanged, its row is where it was; moved, the
    /// row its content is at now comes back through [`Turn::row_of`].
    #[must_use]
    pub const fn layout(&self) -> u64 {
        self.layout
    }

    /// Rows the last layout change rebuilt — every row for a whole
    /// re-wrap, one card's own for a toggle, a pane or a strip on it.
    #[must_use]
    pub const fn rebuilt_rows(&self) -> usize {
        self.rebuilt
    }

    /// Where the window sits in the turn's *content* when its first
    /// visible row is `row`: the line under that row and how far into
    /// the line's wrap it is. A row past the end names the last line;
    /// an empty turn has no anchor.
    #[must_use]
    pub fn anchor_at(&self, row: usize) -> Option<Anchor> {
        let index = self
            .lines
            .partition_point(|line| line.row_start <= row)
            .checked_sub(1)?;
        Some(Anchor::new(index, row - self.lines[index].row_start))
    }

    /// The row `anchor`'s content sits at now — its line's first row
    /// plus the anchor's offset, clamped to the rows the line has at
    /// this wrap, so a window that was inside a card's body when the
    /// card folded lands on its head. A line the turn does not have
    /// resolves to the end.
    #[must_use]
    pub fn row_of(&self, anchor: Anchor) -> usize {
        let Some(line) = self.lines.get(anchor.line()) else {
            return self.rows.len();
        };
        let end = self
            .lines
            .get(anchor.line() + 1)
            .map_or(self.rows.len(), |next| next.row_start);
        let rows = end.saturating_sub(line.row_start).max(1);
        line.row_start + anchor.row().min(rows - 1)
    }

    /// The receipt `id` earned, once its call completed.
    #[must_use]
    pub fn receipt(&self, id: &str) -> Option<&Receipt> {
        self.receipts.get(id)
    }

    /// Record (or correct) the receipt call `id` earned. Rendering-only
    /// data: no row moves and nothing re-wraps.
    pub fn set_receipt(&mut self, id: &str, receipt: Receipt) {
        self.receipts.insert(id.to_owned(), receipt);
    }

    /// The `+added −removed` of the edit card `id`, counted off the
    /// diff body the card holds — `None` for a call with no diff. One
    /// classifier decides what a body line is (`turn::diff_kind`), so
    /// this count and the rows on screen cannot disagree.
    #[must_use]
    pub fn diff_stats(&self, id: &str) -> Option<(u32, u32)> {
        let line =
            self.lines.iter().rev().find(
                |line| matches!(&line.kind, LineKind::Edit { id: known, .. } if known == id),
            )?;
        let mut added = 0u32;
        let mut removed = 0u32;
        for body in line.text.lines() {
            match super::diff_kind::diff_kind(body) {
                RowKind::DiffAdd => added = added.saturating_add(1),
                RowKind::DiffRemove | RowKind::Conflict => removed = removed.saturating_add(1),
                _ => {}
            }
        }
        Some((added, removed))
    }

    /// Draw code highlighting for `mode` from here on; a change re-wraps
    /// (and re-highlights) every line once.
    pub fn set_mode(&mut self, mode: ThemeMode) {
        if self.mode != mode {
            self.mode = mode;
            self.rewrap(self.width);
        }
    }

    /// Draw the status glyphs from `icons` from here on; a change
    /// re-wraps every line once.
    pub fn set_icons(&mut self, icons: &'static Icons) {
        if self.icons != icons {
            self.icons = icons;
            self.rewrap(self.width);
        }
    }

    /// Measure image blocks against `cell` from here on — the terminal
    /// answered `CSI 16 t`, or the font changed. A change re-measures
    /// every image and re-wraps the turn once; an image is as many rows
    /// tall as its footprint, so its rows move with the cell.
    pub fn set_cell_size(&mut self, cell: CellSize) {
        if self.cell == cell {
            return;
        }
        self.cell = cell;
        for line in &mut self.lines {
            if let LineKind::Image(image) = &mut line.kind {
                image.set_cell_size(cell);
            }
        }
        self.rewrap(self.width);
    }

    /// The cell size image blocks are measured against.
    #[must_use]
    pub const fn cell_size(&self) -> CellSize {
        self.cell
    }

    /// Let image blocks stand up to `rows` tall from here on — the
    /// surface has the room for more than a terminal's twenty, or
    /// less. A change re-measures every image and re-wraps the turn
    /// once, exactly as a new cell size does; the same cap again is
    /// nothing.
    pub fn set_row_cap(&mut self, rows: u16) {
        let rows = rows.max(1);
        if self.row_cap == rows {
            return;
        }
        self.row_cap = rows;
        for line in &mut self.lines {
            if let LineKind::Image(image) = &mut line.kind {
                image.set_row_cap(rows);
            }
        }
        self.rewrap(self.width);
    }

    /// The tallest an image block may stand, in rows.
    #[must_use]
    pub const fn row_cap(&self) -> u16 {
        self.row_cap
    }

    /// An image the agent sent: one new line, as many rows as it takes.
    /// The image is re-measured against the turn's cell size first, so
    /// a block decoded before the terminal answered still lands right.
    pub fn push_image(&mut self, mut image: StreamImage, alt: &str) {
        self.close();
        image.set_cell_size(self.cell);
        image.set_row_cap(self.row_cap);
        self.push_line(LineKind::Image(image), alt);
    }

    /// The `index`-th image block of tool call `id`: its own line after
    /// the call's card, as many rows as the image takes.
    ///
    /// The line is keyed by `(id, index)`, so a call re-sending the same
    /// block on a later update lands on the line it already pushed —
    /// the card's own rows never move, and no second copy appears.
    /// `image` is `None` for a block this build cannot decode; the line
    /// is then the note `text` carries, so nothing is silently dropped.
    ///
    /// An update that stops sending a block does not retract it: a
    /// picture the reader has seen stays where they saw it.
    pub fn set_tool_image(
        &mut self,
        id: &str,
        index: usize,
        image: Option<StreamImage>,
        text: &str,
    ) {
        let kind = match image {
            Some(mut image) => {
                image.set_cell_size(self.cell);
                image.set_row_cap(self.row_cap);
                LineKind::Image(image)
            }
            None => LineKind::Note,
        };
        let key = (id.to_owned(), index);
        if let Some(at) = self
            .lines
            .iter()
            .position(|line| line.source == Some(key.clone()))
        {
            let line = &mut self.lines[at];
            if line.kind == kind && line.text == text {
                return;
            }
            line.kind = kind;
            line.text = text.to_owned();
            // An image's row count moves with its footprint, so the
            // line re-wraps rather than one row being replaced.
            self.rewrap_line(at);
            return;
        }
        self.close();
        self.push_sourced_line(kind, text, Some(key));
    }

    /// The image whose rows cover `row`, with the row's offset inside
    /// the placement and the placement's size in cells — what the view
    /// needs to fill one row of it, and the driver to place a surface.
    #[must_use]
    pub fn image_at_row(&self, row: usize) -> Option<(&StreamImage, u16, (u16, u16))> {
        let index = self
            .lines
            .partition_point(|line| line.row_start <= row)
            .checked_sub(1)?;
        let line = &self.lines[index];
        let LineKind::Image(image) = &line.kind else {
            return None;
        };
        let offset = u16::try_from(row.checked_sub(line.row_start)?).ok()?;
        let (cols, rows) = image.footprint(self.width);
        // `row` past the placement is a row beyond the turn: the last
        // line owns every index above its start, image or not.
        (offset < rows).then_some((image, offset, (cols, rows)))
    }

    /// The width the rows are wrapped to.
    #[must_use]
    pub const fn width(&self) -> u16 {
        self.width
    }

    /// Columns a mono line is held to — the prose width unless the
    /// surface reported a narrower one.
    #[must_use]
    pub const fn mono_width(&self) -> u16 {
        match self.mono {
            Some(cols) if cols < self.width => cols,
            _ => self.width,
        }
    }

    /// Hold mono lines — tool and check heads, fenced code, diffs — to
    /// their own column count, and rewrap.
    ///
    /// A window draws its prose in the proportional face and
    /// its code in the mono one, inside a turn column measured in
    /// PIXELS. One column count cannot serve both: counted for the mono
    /// advance the prose breaks a third short of its measure, counted
    /// for the prose the code overruns the column. So the surface
    /// reports what its own box holds in mono cells and prose keeps the
    /// wider count. `None` — every cell surface — is one width for
    /// everything, exactly as before.
    pub fn set_mono_width(&mut self, cols: Option<u16>) {
        if self.mono == cols {
            return;
        }
        self.mono = cols;
        self.rewrap(self.width);
    }

    /// The columns this line is wrapped to: mono lines answer
    /// [`Turn::mono_width`], prose the full width. A fenced line is
    /// code whatever its kind says.
    const fn columns_for(&self, kind: &LineKind, in_fence: bool) -> u16 {
        if in_fence || !matches!(kind, LineKind::Prose(_)) {
            self.mono_width()
        } else {
            self.width
        }
    }

    /// The rows, top to bottom.
    #[must_use]
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Number of rows.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether nothing has arrived.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// The source lines, in arrival order.
    #[must_use]
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    /// The tool, check or edit line whose rows begin exactly at row
    /// `at`, with how many rows it holds — the card a native feed
    /// floats there. A line whose head is scrolled off the top stays
    /// plain rows; an image and every prose kind never cards.
    #[must_use]
    pub fn card_at_row(&self, at: usize) -> Option<(&Line, usize)> {
        let index = self
            .lines
            .partition_point(|line| line.row_start() <= at)
            .checked_sub(1)?;
        let line = self.lines.get(index)?;
        if line.row_start() != at {
            return None;
        }
        if !matches!(
            line.kind(),
            LineKind::Tool { .. } | LineKind::Edit { .. } | LineKind::Check { .. }
        ) {
            return None;
        }
        let end = self
            .lines
            .get(index + 1)
            .map_or(self.len(), Line::row_start);
        Some((line, end.saturating_sub(at).max(1)))
    }

    /// The prose line whose rows cover row `at`, with how many of its
    /// rows run from `at` to its end — the paragraph a native feed
    /// joins back into one run so the drawing face's shaper breaks it
    /// at the column's measure instead of the grid's column count. Only
    /// a [`LineKind::Prose`] line answers: a card, a check, an image
    /// and the cockpit's own notes keep their rows. A paragraph whose
    /// head is scrolled off the top answers from `at`, so the window
    /// joins what it can see.
    #[must_use]
    pub fn prose_at_row(&self, at: usize) -> Option<(&Line, usize)> {
        if at >= self.len() {
            return None;
        }
        let index = self
            .lines
            .partition_point(|line| line.row_start() <= at)
            .checked_sub(1)?;
        let line = self.lines.get(index)?;
        if !matches!(line.kind(), LineKind::Prose(_)) {
            return None;
        }
        let end = self
            .lines
            .get(index + 1)
            .map_or(self.len(), Line::row_start);
        Some((line, end.saturating_sub(at).max(1)))
    }

    /// The newest thing the agent thought, when it thought anything —
    /// the review's `AGENT NOTE`, its one-line *why*.
    ///
    /// Derived from the turn the cockpit already holds rather than
    /// carried on the ask: an agent's rationale is something it said,
    /// and a note the review asserted separately could differ from what
    /// the feed above it shows. Blank thoughts are skipped, so a note is
    /// a sentence or it is absent.
    #[must_use]
    pub fn last_thought(&self) -> Option<&str> {
        self.lines
            .iter()
            .rev()
            .find(|line| {
                *line.kind() == LineKind::Prose(Voice::Thought) && !line.text().trim().is_empty()
            })
            .map(Line::text)
    }

    /// A delta of prose in `voice`: appended to the open line of that
    /// voice, split on newlines into further lines.
    pub fn push_text(&mut self, voice: Voice, delta: &str) {
        if delta.is_empty() {
            return;
        }
        let pieces: Vec<&str> = delta.split('\n').collect();
        let last = pieces.len() - 1;
        for (i, piece) in pieces.into_iter().enumerate() {
            if i > 0 {
                self.close();
                if i == last && piece.is_empty() {
                    break;
                }
            }
            self.append(voice, piece);
        }
    }

    /// A tool call announced: one new line, collapsed.
    pub fn push_tool(&mut self, id: &str, title: &str, verb: ToolVerb, status: ToolStatus) {
        self.close();
        self.push_line(
            LineKind::Tool {
                id: id.to_owned(),
                status,
                verb,
                duration_ms: None,
                expanded: false,
            },
            title,
        );
    }

    /// The accent of the line whose rows cover `row` — the gutter bar
    /// the view draws beside it.
    #[must_use]
    pub fn accent_at_row(&self, row: usize) -> Accent {
        let Some(index) = self
            .lines
            .partition_point(|line| line.row_start <= row)
            .checked_sub(1)
        else {
            return Accent::Meta;
        };
        Accent::of(&self.lines[index].kind)
    }

    /// Expand or collapse the foldable card — a tool call or a check —
    /// whose rows cover `row`. Returns whether one sits there.
    pub fn toggle_at_row(&mut self, row: usize) -> bool {
        let Some(index) = self
            .lines
            .partition_point(|line| line.row_start <= row)
            .checked_sub(1)
        else {
            return false;
        };
        self.toggle_line(index)
    }

    /// Expand or collapse the most recent foldable card. Returns whether
    /// one exists.
    pub fn toggle_last(&mut self) -> bool {
        let Some(index) = self.lines.iter().rposition(|line| {
            matches!(
                line.kind,
                LineKind::Tool { .. } | LineKind::Check { .. } | LineKind::Edit { .. }
            )
        }) else {
            return false;
        };
        self.toggle_line(index)
    }

    /// Fold the most recently pushed card that is open — a window's Esc
    /// before it climbs.
    /// Returns whether one was open.
    pub fn collapse_expanded(&mut self) -> bool {
        let Some(index) = self.lines.iter().rposition(|line| {
            matches!(
                line.kind,
                LineKind::Tool { expanded: true, .. }
                    | LineKind::Check { expanded: true, .. }
                    | LineKind::Edit { expanded: true, .. }
            )
        }) else {
            return false;
        };
        self.toggle_line(index)
    }

    /// Flip the expansion of the card at `index`; the row count
    /// changes, so the card re-wraps and everything after it shifts.
    fn toggle_line(&mut self, index: usize) -> bool {
        let Some(line) = self.lines.get_mut(index) else {
            return false;
        };
        let (LineKind::Tool { expanded, .. }
        | LineKind::Check { expanded, .. }
        | LineKind::Edit { expanded, .. }) = &mut line.kind
        else {
            return false;
        };
        *expanded = !*expanded;
        self.rewrap_line(index);
        true
    }

    /// A tool call changed: its collapsed row is replaced in place — or,
    /// while it is expanded, everything re-wraps, since the detail's row
    /// count can change. An edit card counts as its call: a status
    /// update lands on it (its title is the path, so a new title is
    /// ignored, as is a duration). Returns whether the call was known.
    pub fn update_tool(
        &mut self,
        id: &str,
        title: Option<&str>,
        status: Option<ToolStatus>,
        duration: Option<u64>,
    ) -> bool {
        let Some(index) = self.tool_line(id) else {
            return false;
        };
        let line = &mut self.lines[index];
        if let (Some(title), LineKind::Tool { .. }) = (title, &line.kind) {
            line.text = title.to_owned();
        }
        if let (
            Some(status),
            LineKind::Tool { status: known, .. } | LineKind::Edit { status: known, .. },
        ) = (status, &mut line.kind)
        {
            *known = status;
        }
        if let (Some(ms), LineKind::Tool { duration_ms, .. }) = (duration, &mut line.kind) {
            *duration_ms = Some(ms);
        }
        self.refresh_line(index);
        true
    }

    /// The most recent tool call or edit line of call `id`.
    fn tool_line(&self, id: &str) -> Option<usize> {
        self.lines.iter().rposition(|line| {
            matches!(
                &line.kind,
                LineKind::Tool { id: known, .. } | LineKind::Edit { id: known, .. }
                    if known == id
            )
        })
    }

    /// The current source line for a stable tool-call identity.
    #[must_use]
    pub fn tool(&self, id: &str) -> Option<&Line> {
        self.tool_line(id).map(|index| &self.lines[index])
    }

    /// The record tool call `id` keeps — what it said past its row.
    /// `None` when no call of that id is on the turn.
    #[must_use]
    pub fn tool_record(&self, id: &str) -> Option<&ToolRecord> {
        self.tool_line(id).map(|index| &self.lines[index].record)
    }

    /// Fold `record` into the one tool call `id`'s line keeps: what the
    /// record carries replaces, what it leaves out stays, and the whole
    /// stays under [`ToolRecord::CAP`]. Data beside the line, never a
    /// row — nothing re-wraps, unless the record re-kinds a tool row,
    /// whose verb column then redraws. Returns whether the call is
    /// known.
    pub fn set_tool_record(&mut self, id: &str, record: ToolRecord) -> bool {
        let Some(index) = self.tool_line(id) else {
            return false;
        };
        let line = &mut self.lines[index];
        let had_pane = line.record.terminal.is_some();
        line.record.absorb(record);
        if let (Some(kind), LineKind::Tool { verb, .. }) = (line.record.kind, &mut line.kind)
            && *verb != kind
        {
            *verb = kind;
            self.refresh_line(index);
        }
        // A terminal named on a card already open grows the card by
        // its pane, which is a row-count change: the card re-wraps.
        if !had_pane && pane_rows(&self.lines[index]) > 0 {
            self.rewrap_line(index);
        }
        true
    }

    /// The gate `name`'s newest check card embeds terminal `id` — the
    /// screen its run streamed — so the card opens into the pane.
    /// Returns whether a check of that name is on the turn.
    pub fn set_check_terminal(&mut self, name: &str, id: &str) -> bool {
        let Some(index) = self
            .lines
            .iter()
            .rposition(|line| matches!(&line.kind, LineKind::Check { name: n, .. } if n == name))
        else {
            return false;
        };
        self.lines[index].record.terminal = Some(id.to_owned());
        if pane_rows(&self.lines[index]) > 0 {
            self.rewrap_line(index);
        }
        true
    }

    /// The program on terminal `id` reports its progress, or stopped:
    /// the newest card embedding it grows a strip row under its head,
    /// or gives it back — a row-count change on an open card, so it
    /// re-wraps. Returns whether anything changed.
    pub fn set_terminal_strip(&mut self, id: &str, on: bool) -> bool {
        let Some(index) = self
            .lines
            .iter()
            .rposition(|line| line.record.terminal.as_deref() == Some(id))
        else {
            return false;
        };
        if self.lines[index].strip == on {
            return false;
        }
        self.lines[index].strip = on;
        if pane_rows(&self.lines[index]) > 0 || self.is_open(index) {
            self.rewrap_line(index);
        }
        true
    }

    /// Whether the card embedding terminal `id` carries the strip row —
    /// the first of its pane rows, ahead of the screen's.
    #[must_use]
    pub fn has_strip(&self, id: &str) -> bool {
        self.lines
            .iter()
            .rfind(|line| line.record.terminal.as_deref() == Some(id))
            .is_some_and(|line| line.strip)
    }

    /// Whether line `index` is an open card.
    fn is_open(&self, index: usize) -> bool {
        matches!(
            self.lines[index].kind,
            LineKind::Tool { expanded: true, .. } | LineKind::Check { expanded: true, .. }
        )
    }

    /// The running card's terminal — the newest line whose record names
    /// one, open or folded, running or ended. `None` when no card
    /// on the turn ran on a pseudo-terminal.
    #[must_use]
    pub fn running_terminal(&self) -> Option<&str> {
        self.lines
            .iter()
            .rev()
            .find_map(|line| line.record.terminal.as_deref())
    }

    /// Whether the newest record naming this terminal says its work ended.
    #[must_use]
    pub fn terminal_finished(&self, id: &str) -> bool {
        self.lines
            .iter()
            .rev()
            .find(|line| line.record.terminal.as_deref() == Some(id))
            .is_some_and(|line| {
                matches!(
                    line.kind,
                    LineKind::Tool {
                        status: ToolStatus::Completed | ToolStatus::Failed,
                        ..
                    } | LineKind::Edit {
                        status: ToolStatus::Completed | ToolStatus::Failed,
                        ..
                    } | LineKind::Check { .. }
                )
            })
    }

    /// The title of the newest card that ran on terminal `id` — the
    /// command, as the dock's grip names it.
    #[must_use]
    pub fn terminal_title(&self, id: &str) -> Option<&str> {
        self.lines
            .iter()
            .rev()
            .find(|line| line.record.terminal.as_deref() == Some(id))
            .map(|line| line.text.as_str())
    }

    /// Whether the newest card that ran on terminal `id` is a gate's
    /// check rather than the agent's call — what a held kill must know
    /// to reach the right producer: a gate's child is
    /// the loop driver's, the agent's terminal the transport's.
    #[must_use]
    pub fn terminal_is_gate(&self, id: &str) -> bool {
        self.lines
            .iter()
            .rev()
            .find(|line| line.record.terminal.as_deref() == Some(id))
            .is_some_and(|line| matches!(line.kind, LineKind::Check { .. }))
    }

    /// The terminal whose pane rows cover `row`, with the row's offset
    /// inside the pane — what the view needs to fill one row of it from
    /// the program's screen. The pane is the last [`Turn::PANE_ROWS`]
    /// rows of an open card that embeds a terminal.
    #[must_use]
    pub fn pane_at_row(&self, row: usize) -> Option<(&str, usize)> {
        let index = self
            .lines
            .partition_point(|line| line.row_start <= row)
            .checked_sub(1)?;
        let line = &self.lines[index];
        let rows = pane_rows(line);
        if rows == 0 {
            return None;
        }
        let end = self
            .lines
            .get(index + 1)
            .map_or(self.rows.len(), |next| next.row_start);
        let start = end.checked_sub(rows)?;
        let offset = row.checked_sub(start)?;
        (offset < rows).then(|| (line.record.terminal.as_deref().unwrap_or_default(), offset))
    }

    /// A workbench edit announced: one new line, collapsed — the diff
    /// body waits behind the fold.
    pub fn push_edit(&mut self, id: &str, path: &str, status: ToolStatus, body: &str) {
        self.close();
        self.push_line(
            LineKind::Edit {
                id: id.to_owned(),
                path: path.to_owned(),
                status,
                expanded: false,
                undone: false,
            },
            body,
        );
    }

    /// Preserve the provider's title independently of an edit's diff body.
    pub fn set_tool_title(&mut self, id: &str, title: &str) {
        if let Some(index) = self.tool_line(id) {
            self.lines[index].title = Some(title.to_owned());
        }
    }

    /// A diff landed for a call: the plain tool row it was announced as
    /// becomes the edit card (an existing card takes the new body and
    /// path); an unknown id pushes a fresh card. Without a status the
    /// line keeps the one it has (a fresh card starts pending).
    pub fn set_edit(&mut self, id: &str, path: &str, status: Option<ToolStatus>, body: &str) {
        let Some(index) = self.tool_line(id) else {
            return self.push_edit(id, path, status.unwrap_or(ToolStatus::Pending), body);
        };
        let line = &mut self.lines[index];
        if line.title.is_none() && matches!(line.kind, LineKind::Tool { .. }) {
            line.title = Some(line.text.clone());
        }
        let (prior, expanded, undone) = match &line.kind {
            LineKind::Tool {
                status, expanded, ..
            } => (*status, *expanded, false),
            LineKind::Edit {
                status,
                expanded,
                undone,
                ..
            } => (*status, *expanded, *undone),
            _ => return,
        };
        line.kind = LineKind::Edit {
            id: id.to_owned(),
            path: path.to_owned(),
            status: status.unwrap_or(prior),
            expanded,
            undone,
        };
        line.text = body.to_owned();
        self.refresh_line(index);
    }

    /// The workbench journal rewound an edit: its card reads ` · undone`
    /// and drops its Undo button. Returns whether the card was known.
    pub fn undo_edit(&mut self, id: &str) -> bool {
        let Some(index) = self.lines.iter().rposition(
            |line| matches!(&line.kind, LineKind::Edit { id: known, .. } if known == id),
        ) else {
            return false;
        };
        if let LineKind::Edit { undone, .. } = &mut self.lines[index].kind {
            *undone = true;
        }
        self.refresh_line(index);
        true
    }

    /// The edit card whose rows cover `row`, as (id, path, undone) —
    /// how a click on its buttons finds the edit they act on.
    #[must_use]
    pub fn edit_at_row(&self, row: usize) -> Option<(&str, &str, bool)> {
        let index = self
            .lines
            .partition_point(|line| line.row_start <= row)
            .checked_sub(1)?;
        match &self.lines[index].kind {
            LineKind::Edit {
                id, path, undone, ..
            } => Some((id.as_str(), path.as_str(), *undone)),
            _ => None,
        }
    }

    /// Re-render the changed line at `index`: its one collapsed row is
    /// replaced in place; while it is expanded the card re-wraps, since
    /// the detail's row count can change.
    fn refresh_line(&mut self, index: usize) {
        let line = &self.lines[index];
        if matches!(
            line.kind,
            LineKind::Tool { expanded: true, .. }
                | LineKind::Check { expanded: true, .. }
                | LineKind::Edit { expanded: true, .. }
        ) {
            self.rewrap_line(index);
            return;
        }
        let columns = self.columns_for(&line.kind, false);
        let rows = PixelWrap::wrap(
            self.pixel_wrap,
            &line.kind,
            &line.text,
            false,
            None,
            self.icons,
            columns,
        );
        if let (Some(slot), Some(row)) =
            (self.rows.get_mut(line.row_start), rows.into_iter().next())
        {
            *slot = row;
        }
    }

    /// A quality-gate check recorded: one new line, collapsed; the
    /// details wait behind the fold. The gate's `command`, when the
    /// recorder kept one, rides the line's record as its input — data
    /// a detail names, never a row the grid draws.
    pub fn push_check(
        &mut self,
        name: &str,
        command: Option<&str>,
        passed: bool,
        exit_code: Option<i64>,
        duration_ms: Option<u64>,
        details: &str,
    ) {
        self.close();
        self.push_line(
            LineKind::Check {
                name: name.to_owned(),
                passed,
                exit_code,
                duration_ms,
                expanded: false,
            },
            details,
        );
        if let (Some(command), Some(line)) = (command, self.lines.last_mut()) {
            line.record.input = Some(serde_json::Value::String(command.to_owned()));
        }
    }

    /// A think-and-ship event: one new line, its glyph already in the
    /// text.
    pub fn push_event(&mut self, text: &str) {
        self.close();
        self.push_line(LineKind::Event, text);
    }

    /// A note from the cockpit: one new line.
    pub fn push_note(&mut self, text: &str) {
        self.close();
        self.push_line(LineKind::Note, text);
    }

    /// An error: one new line.
    pub fn push_error(&mut self, text: &str) {
        self.close();
        self.push_line(LineKind::Error, text);
    }

    /// Wrap every line again at `width`, replaying the fence state — and
    /// each closed code line through a fresh highlighter — front to back.
    pub fn rewrap(&mut self, width: u16) {
        self.width = width;
        self.rows.clear();
        self.fence = None;
        let icons = self.icons;
        let last = self.lines.len().checked_sub(1);
        for i in 0..self.lines.len() {
            let tail_open = self.open && Some(i) == last;
            self.lines[i].row_start = self.rows.len();
            let in_fence = self.fence.is_some();
            let columns = self.columns_for(&self.lines[i].kind, in_fence);
            let highlight = if tail_open { None } else { self.fence.as_mut() };
            let line = &self.lines[i];
            self.rows.extend(PixelWrap::wrap(
                self.pixel_wrap,
                &line.kind,
                &line.text,
                in_fence,
                highlight,
                icons,
                columns,
            ));
            self.rows.extend(
                std::iter::repeat_with(|| Row::new(RowKind::Pane, "")).take(pane_rows(line)),
            );
            if !tail_open {
                self.pass_fence(i);
            }
        }
        self.layout = self.layout.wrapping_add(1);
        self.rebuilt = self.rows.len();
    }

    /// Wrap line `index` again on its own and splice its rows in place
    /// of the ones it had, every later line's start shifted by the
    /// difference — what a card opening, folding, growing a pane or a
    /// strip costs: the card's rows, never the turn's. Only a line the
    /// fence state cannot change is wrapped here — a card, an image, a
    /// note — so a prose line takes the whole re-wrap instead, fence
    /// and highlighting replayed.
    fn rewrap_line(&mut self, index: usize) {
        let Some(line) = self.lines.get(index) else {
            return;
        };
        if matches!(line.kind, LineKind::Prose(_)) {
            self.rewrap(self.width);
            return;
        }
        let start = line.row_start;
        let end = self
            .lines
            .get(index + 1)
            .map_or(self.rows.len(), |next| next.row_start);
        let columns = self.columns_for(&line.kind, false);
        let mut fresh = PixelWrap::wrap(
            self.pixel_wrap,
            &line.kind,
            &line.text,
            false,
            None,
            self.icons,
            columns,
        );
        fresh.extend(std::iter::repeat_with(|| Row::new(RowKind::Pane, "")).take(pane_rows(line)));
        let (had, rebuilt) = (end.saturating_sub(start), fresh.len());
        self.rows.splice(start..end, fresh);
        for later in &mut self.lines[index + 1..] {
            later.row_start = (later.row_start + rebuilt).saturating_sub(had);
        }
        self.layout = self.layout.wrapping_add(1);
        self.rebuilt = rebuilt;
    }

    /// Close the open line, if any: the fence state moves past it, and a
    /// closed line of a fenced block takes its syntect spans — the one
    /// highlight pass that line gets.
    fn close(&mut self) {
        if !self.open {
            return;
        }
        self.open = false;
        let Some(index) = self.lines.len().checked_sub(1) else {
            return;
        };
        if !matches!(self.lines[index].kind, LineKind::Prose(_)) {
            return;
        }
        if self.fence.is_some() && !opens_fence(&self.lines[index].text) {
            let icons = self.icons;
            // In a fence: code, held to the mono measure.
            let columns = self.mono_width();
            let line = &self.lines[index];
            let rows = PixelWrap::wrap(
                self.pixel_wrap,
                &line.kind,
                &line.text,
                true,
                self.fence.as_mut(),
                icons,
                columns,
            );
            self.rows.truncate(line.row_start);
            self.rows.extend(rows);
        }
        self.pass_fence(index);
    }

    /// Move the fence state past the closed prose line at `index`: a
    /// fence line opens a block (with its language) or closes the one
    /// running.
    fn pass_fence(&mut self, index: usize) {
        let line = &self.lines[index];
        if !matches!(line.kind, LineKind::Prose(_)) || !opens_fence(&line.text) {
            return;
        }
        self.fence = match self.fence.take() {
            Some(_) => None,
            None => Some(Highlight::for_fence(&line.text, self.mode)),
        };
    }

    /// Append `piece` to the open line of `voice`, or open one. The open
    /// line renders plain until it closes.
    fn append(&mut self, voice: Voice, piece: &str) {
        let same_voice = self
            .lines
            .last()
            .is_some_and(|line| line.kind == LineKind::Prose(voice));
        if self.open && same_voice {
            let in_fence = self.fence.is_some();
            let columns = self.columns_for(
                &self.lines.last().expect("an open line exists").kind,
                in_fence,
            );
            let line = self.lines.last_mut().expect("an open line exists");
            line.text.push_str(piece);
            self.rows.truncate(line.row_start);
            let icons = self.icons;
            self.rows.extend(PixelWrap::wrap(
                self.pixel_wrap,
                &line.kind,
                &line.text,
                in_fence,
                None,
                icons,
                columns,
            ));
        } else {
            self.close();
            self.push_line(LineKind::Prose(voice), piece);
            self.open = true;
        }
    }

    fn push_line(&mut self, kind: LineKind, text: &str) {
        self.push_sourced_line(kind, text, None);
    }

    /// Append one line, tagged with the tool-call content it came from.
    fn push_sourced_line(&mut self, kind: LineKind, text: &str, source: Option<(String, usize)>) {
        let row_start = self.rows.len();
        let columns = self.columns_for(&kind, self.fence.is_some());
        self.rows.extend(PixelWrap::wrap(
            self.pixel_wrap,
            &kind,
            text,
            self.fence.is_some(),
            None,
            self.icons,
            columns,
        ));
        self.lines.push(Line {
            kind,
            text: text.to_owned(),
            title: None,
            row_start,
            source,
            record: ToolRecord::default(),
            strip: false,
        });
    }
}

/// How many blank [`RowKind::Pane`] rows `line` takes under its detail:
/// [`Turn::PANE_ROWS`] for an open card that embeds a terminal, none
/// for a folded card or one with no terminal. The rows carry no text —
/// the view fills them from the program's screen — but they exist so
/// the pane scrolls, is measured and is found again like any other
/// block.
fn pane_rows(line: &Line) -> usize {
    let open = matches!(
        line.kind,
        LineKind::Tool { expanded: true, .. } | LineKind::Check { expanded: true, .. }
    ) && line.record.terminal.is_some();
    if open {
        Turn::PANE_ROWS + usize::from(line.strip)
    } else {
        0
    }
}

#[cfg(test)]
#[path = "stream.tests.rs"]
mod tests;
