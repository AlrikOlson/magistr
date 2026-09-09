//! The terminal dock — the running card's screen raised over the session.

use super::eased::Eased;
use super::term::Term;

/// The terminal panel raised over the session feed.
///
/// The panel keeps its terminal ID, dragged height, maximized state, and
/// animation offset. `⌘⇧↵` maximizes it and `esc` lowers it before navigating.
/// The preferred height is stored per project, initially a third of the stage.
///
/// Height is clamped between [`Term::PANE_ROWS`] and [`Term::ROWS`]. The rise
/// is an [`Eased`] fraction sampled at [`Model::now`](super::Model::now);
/// reduced motion settles it immediately. A lowering panel retains its
/// terminal until the animation finishes.
#[derive(Clone, Debug, PartialEq)]
pub struct TermDock {
    term: Option<String>,
    up: bool,
    rows: u16,
    maximized: bool,
    /// The rows a maximized dock takes — the stage's, measured when it
    /// was maximized.
    max: u16,
    /// A grip drag: where the pointer was pressed (in rows, fractional)
    /// and the rows the dock had then.
    drag: Option<(f32, u16)>,
    rise: Eased,
}

impl TermDock {
    /// The least rows the dock takes: a card's own pane.
    pub const MIN_ROWS: u16 = Term::PANE_ROWS as u16;
    /// The most rows the dock takes: the screen's.
    pub const MAX_ROWS: u16 = Term::ROWS;

    /// A dock that is down.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            term: None,
            up: false,
            rows: Self::MIN_ROWS,
            maximized: false,
            max: Self::MAX_ROWS,
            drag: None,
            rise: Eased::new(),
        }
    }

    /// Raise terminal `id`'s screen to `rows` (clamped), rising from
    /// the edge at step `now` — at once with `animate` off. A dock
    /// already up moves to the new terminal and height.
    pub fn raise(&mut self, id: &str, rows: u16, now: f32, animate: bool) {
        if !self.rise.is_seeded() {
            self.rise.retarget(0.0, now, false);
        }
        self.term = Some(id.to_owned());
        self.up = true;
        self.maximized = false;
        self.rows = Self::clamp(rows);
        self.rise.retarget(1.0, now, animate);
    }

    /// Lower the dock: it glides back to the edge from wherever it is.
    /// Returns whether it was up.
    pub fn lower(&mut self, now: f32, animate: bool) -> bool {
        if !self.up {
            return false;
        }
        self.up = false;
        self.drag = None;
        self.rise.retarget(0.0, now, animate);
        true
    }

    /// Maximize the dock to `max` rows (clamped): the rail stays, the
    /// dragged height is kept for the way back. Nothing while down.
    pub fn maximize(&mut self, max: u16, now: f32, animate: bool) {
        if !self.up {
            return;
        }
        self.max = Self::clamp(max);
        self.maximized = true;
        self.drag = None;
        self.rise.retarget(1.0, now, animate);
    }

    /// Back from maximized to the dragged height.
    pub const fn restore(&mut self) {
        self.maximized = false;
    }

    /// Whether the dock is raised (or rising).
    #[must_use]
    pub const fn is_up(&self) -> bool {
        self.up
    }

    /// Whether the dock is maximized.
    #[must_use]
    pub const fn is_maximized(&self) -> bool {
        self.maximized
    }

    /// The terminal the dock holds — set on the first raise, kept
    /// through a lowering so the glide down has a screen.
    #[must_use]
    pub fn id(&self) -> Option<&str> {
        self.term.as_deref()
    }

    /// The terminal the dock shows at step `now`: `Some` while any of
    /// it is above the edge — raised, rising or still lowering.
    #[must_use]
    pub fn term(&self, now: f32) -> Option<&str> {
        (self.height_at(now) > 0)
            .then_some(self.term.as_deref())
            .flatten()
    }

    /// The dragged (or remembered) height, in rows.
    #[must_use]
    pub const fn rows(&self) -> u16 {
        self.rows
    }

    /// The rows the dock is aiming at: the dragged height, or the
    /// stage's while maximized.
    #[must_use]
    pub const fn target_rows(&self) -> u16 {
        if self.maximized { self.max } else { self.rows }
    }

    /// The rows the dock shows at step `now`: the target scaled by the
    /// rise, rounded to whole rows — a grid has no half row on either
    /// surface — and zero once lowered and settled.
    #[must_use]
    pub fn height_at(&self, now: f32) -> usize {
        let fraction = self.rise.at(now).clamp(0.0, 1.0);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let rows = (f32::from(self.target_rows()) * fraction).round() as usize;
        rows
    }

    /// The rows the dock takes off the stage at step `now`: its height
    /// and the grip row above it, or nothing while down.
    #[must_use]
    pub fn taken(&self, now: f32) -> usize {
        match self.height_at(now) {
            0 => 0,
            rows => rows + 1,
        }
    }

    /// Whether the rise is still gliding at step `now`.
    #[must_use]
    pub fn is_settling(&self, now: f32) -> bool {
        self.rise.is_settling(now)
    }

    /// A press on the grip at `y` (rows from the top, fractional):
    /// the next drags resize the dock. Nothing while down.
    pub fn begin_drag(&mut self, y: f32) {
        if self.up {
            self.drag = Some((y, self.target_rows()));
        }
    }

    /// The pointer moved to `y` during a drag: the dock is as many rows
    /// taller as the grip went up, clamped. Returns the new height when
    /// it changed. A drag un-maximizes — the height is the reader's now.
    pub fn drag_to(&mut self, y: f32) -> Option<u16> {
        let (from, rows) = self.drag?;
        #[allow(clippy::cast_possible_truncation)]
        let delta = (from - y).round() as i32;
        let rows = Self::clamp(u16::try_from((i32::from(rows) + delta).max(0)).unwrap_or(u16::MAX));
        self.maximized = false;
        if rows == self.rows {
            return None;
        }
        self.rows = rows;
        Some(rows)
    }

    /// The press ended: the height to remember, when a drag was on.
    pub fn end_drag(&mut self) -> Option<u16> {
        self.drag.take().map(|_| self.rows)
    }

    /// Whether the grip is being dragged.
    #[must_use]
    pub const fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// `rows` held to the dock's limits.
    #[must_use]
    pub const fn clamp(rows: u16) -> u16 {
        if rows < Self::MIN_ROWS {
            Self::MIN_ROWS
        } else if rows > Self::MAX_ROWS {
            Self::MAX_ROWS
        } else {
            rows
        }
    }
}

impl Default for TermDock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "term_dock.tests.rs"]
mod tests;
