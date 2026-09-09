//! What `update` compares across a fold to find the changes that move.

use super::stage_tab::StageTab;
use crate::widget_id::WidgetId;

/// What `update` compares across a fold to find the changes that move:
/// the stage tab, how many cards the dock holds, the rail's progress,
/// the roadmap's counts, and — for the arrivals and the finishes
///  — how many rows the turn holds, how many gate LEDs have
/// settled, and where the plan's completed tasks stand.
///
/// Every field but one is a *count*, never an identity, which is what
/// keeps [`Motion`](super::Motion) `Copy`: a count that grew names the
/// range of items that are new (everything from the old count up), so
/// the stamp needs an index rather than a map keyed by item. The
/// exception is [`MotionKey::hover`], and it is `Copy` too — a
/// [`WidgetId`] is a `u32`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MotionKey {
    /// The stage tab.
    pub tab: StageTab,
    /// Cards on the dock.
    pub dock: usize,
    /// The rail's `(done, total)`.
    pub rail: (u32, u32),
    /// The roadmap's `(done, active)`.
    pub roadmap: (usize, usize),
    /// Rows the turn holds.
    pub rows: usize,
    /// Gate LEDs that have settled — passed or failed, not still running.
    pub gates: usize,
    /// The plan's `(tasks completed, position of the last completed one)`.
    pub tasks: (usize, usize),
    /// Steps the think trace holds.
    pub trace: usize,
    /// Samples the header's activity monitor shows, capped at the
    /// slot's width — see [`Motion::spark_sweep`](super::Motion::spark_sweep)
    /// for why the cap belongs here and not in the motion layer.
    pub activity: usize,
    /// The widget under the pointer — **only on a window**
    /// ([`Surface::Native`](super::Surface)), and `None` on a terminal
    /// whatever the pointer is over.
    ///
    /// It is here so that a hover *change* is news the motion layer can
    /// stamp, which is what the tooltip's dwell is measured from. The
    /// surface gate is what keeps the terminal's idle guarantee exactly
    /// as it was: a cockpit that shows no tooltip must not spend a tick
    /// timing one, and answering `None` there means the key cannot
    /// change and no stamp is ever taken.
    pub hover: Option<WidgetId>,
    /// Whether the rail is expanded — **only on a window**, `None` on a
    /// terminal for the reason [`MotionKey::hover`] is: a change here
    /// is what the rail's chrome fade is stamped from, and a terminal
    /// that cannot change it never takes the stamp.
    pub rail_open: Option<bool>,
}
