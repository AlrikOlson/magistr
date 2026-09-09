//! One row's mark on the pane's time rail.

use super::model::Model;
use crate::theme::Icons;

/// What the pane's time rail says beside one row: a tick where a second begins, a dotted break
/// where the program was silent for more than a second before the row,
/// and a stamp — the row's time from the program's first byte, `m:ss`
/// — only after a silence, at a ten-second mark, or on the program's
/// first row. Never a column of numbers: most rows are quiet.
///
/// A pure function of three ticks — the program's first, the previous
/// row's and this row's — at the cockpit's tick period, so both
/// surfaces read the same marks from the same stamps and no clock is
/// consulted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RailMark {
    /// The row begins a new second.
    pub tick: bool,
    /// The program was silent for more than a second before the row.
    pub silence: bool,
    /// The row's time from the program's first byte, in whole seconds,
    /// when the rail spells it out.
    pub stamp: Option<u64>,
}

impl RailMark {
    /// The rail's width in cells: five for the stamp, one for the mark,
    /// one of air before the program's own cells.
    pub const COLS: u16 = 7;
    /// The silence that breaks the rail, in milliseconds — more than a
    /// second.
    const SILENCE_MS: u64 = 1_000;
    /// A stamp every this many seconds, silence or not.
    const STAMP_EVERY_SECS: u64 = 10;

    /// The mark for a row that arrived on tick `at`, after the row
    /// before it arrived on `prev` (`None` for the program's first
    /// row), the program having started on tick `started`.
    #[must_use]
    pub fn of(started: u64, prev: Option<u64>, at: u64) -> Self {
        let secs = Self::secs(started, at);
        let Some(prev) = prev else {
            return Self {
                tick: true,
                silence: false,
                stamp: Some(secs),
            };
        };
        let before = Self::secs(started, prev);
        let silence = at.saturating_sub(prev) * Self::period_ms() > Self::SILENCE_MS;
        let tick = secs != before;
        let decade = secs / Self::STAMP_EVERY_SECS != before / Self::STAMP_EVERY_SECS;
        Self {
            tick,
            silence,
            stamp: (silence || decade).then_some(secs),
        }
    }

    /// Whether the rail shows nothing beside the row.
    #[must_use]
    pub const fn is_quiet(&self) -> bool {
        !self.tick && !self.silence && self.stamp.is_none()
    }

    /// The rail's cells for the row, [`RailMark::COLS`] wide: the stamp
    /// right-aligned, then the mark — the break's dotted glyph over the
    /// tick's — then one cell of air.
    #[must_use]
    pub fn text(&self, icons: &Icons) -> String {
        let stamp = self.stamp.map(Self::spell).unwrap_or_default();
        let mark = if self.silence {
            icons.rail_break
        } else if self.tick {
            icons.rail_tick
        } else {
            " "
        };
        format!("{stamp:>5}{mark} ")
    }

    /// `secs` as the rail spells it: `m:ss` under an hour, `h`h`mm`
    /// past it — five cells at most.
    fn spell(secs: u64) -> String {
        if secs < 3_600 {
            format!("{}:{:02}", secs / 60, secs % 60)
        } else {
            format!("{}h{:02}", secs / 3_600, secs % 3_600 / 60)
        }
    }

    /// Whole seconds from `started` to `at`.
    fn secs(started: u64, at: u64) -> u64 {
        at.saturating_sub(started) * Self::period_ms() / 1_000
    }

    /// The cockpit's tick period in milliseconds.
    fn period_ms() -> u64 {
        u64::try_from(Model::TICK_PERIOD.as_millis()).unwrap_or(u64::MAX)
    }
}

#[cfg(test)]
#[path = "rail_mark.tests.rs"]
mod tests;
