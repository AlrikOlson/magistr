//! The cockpit's glyph vocabulary, in two sets.

use crate::pixel::Icon;

/// Every glyph the cockpit draws that means something — one field per
/// idea, never a literal at the call site — so a surface asks for
/// "blocked" rather than for `⊘`, and the whole screen changes
/// vocabulary by changing which set is in force.
///
/// Two sets ship: [`Icons::UNICODE`], the geometric shapes and dingbats
/// any monospace font draws, and [`Icons::NERD`], the Font Awesome block
/// of a Nerd Font. [`super::IconMode`] chooses between them and
/// [`super::Theme`] carries the choice to the view. This extends
/// magistr-theme's `SignalIcons` STANDARD/ASCII pattern from the four
/// signal roles to the cockpit's whole surface area.
///
/// The two sets are the same shape, not the same glyphs: a few marks —
/// the list bullet, the progress bar's cells, the spinner's four
/// quadrant blocks — are block art rather than iconography and stay
/// themselves in both. (The spinner is shared for a hard reason as well
/// as a typographic one: Font Awesome has single spinner glyphs but no
/// rotation sequence, so a Nerd set could only repeat one of them and
/// would not turn.)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Icons {
    // --- tool and task status ---
    /// A tool call announced but not started; a roadmap chunk waiting.
    pub pending: &'static str,
    /// A tool call in flight.
    pub running: &'static str,
    /// A tool call, check or task that succeeded.
    pub done: &'static str,
    /// A tool call, check or task that failed.
    pub failed: &'static str,
    /// A gate still deciding.
    pub waiting: &'static str,
    /// A task that cannot proceed.
    pub blocked: &'static str,
    /// A task passed over.
    pub skipped: &'static str,
    /// A roadmap chunk held up by its dependencies.
    pub stopped: &'static str,
    /// The thing in progress, and the separator between phases.
    pub active: &'static str,
    /// A task planned but not reached.
    pub planned: &'static str,

    // --- disclosure ---
    /// A folded row that can open.
    pub collapsed: &'static str,
    /// An open row that can fold.
    pub expanded: &'static str,
    /// The way back to a live pane's tail — the jump pill's arrow.
    pub latest: &'static str,
    /// A list item's marker — typography, the same in both sets.
    pub bullet: &'static str,
    /// The pane's time rail where a second begins — box art, the same
    /// in both sets.
    pub rail_tick: &'static str,
    /// The pane's time rail where the program fell silent for more
    /// than a second — box art, the same in both sets.
    pub rail_break: &'static str,

    // --- signals ---
    /// Something that needs the human's attention.
    pub warning: &'static str,
    /// Something that has stopped the work. (Not `⛔`: that glyph
    /// is two cells wide, and every icon slot here is one.)
    pub blocker: &'static str,
    /// A roadmap proposal awaiting a decision.
    pub proposal: &'static str,
    /// A stakeholder signal.
    pub signal: &'static str,
    /// A step of the reasoning trace.
    pub think: &'static str,
    /// A pinned step.
    pub pinned: &'static str,
    /// A trace checkpoint.
    pub checkpoint: &'static str,

    // --- roles and chrome ---
    /// The product mark in the header's brand slot.
    pub brand: &'static str,
    /// Where the roadmap currently stands.
    pub here: &'static str,
    /// The prompt's mark, in the user's voice.
    pub prompt: &'static str,
    /// The accelerator sheet.
    pub keys: &'static str,
    /// Undocking a pane into its own window.
    pub undock: &'static str,
    /// Collapsing the rail.
    pub collapse: &'static str,

    // --- files and git ---
    /// A file, and the read-only viewer showing one.
    pub file: &'static str,
    /// An edit awaiting approval.
    pub edit: &'static str,
    /// Lines a diff adds.
    pub added: &'static str,
    /// Lines a diff removes.
    pub removed: &'static str,

    // --- motion ---
    /// The four frames of the spinner that turns while something works:
    /// a quadrant block orbiting the cell — block art, identical in both
    /// sets. Pick one with [`Icons::spin`].
    pub spinner: [&'static str; 4],

    // --- the roadmap's progress bar (block art, identical in both) ---
    /// A cell of the bar that is done.
    pub bar_full: &'static str,
    /// A cell of the bar that is not.
    pub bar_empty: &'static str,

    // --- the sparkline's ramp (block art, identical in both) ---
    /// The eight heights one cell of a cell-drawn sparkline can take,
    /// lowest first — the fallback the header shows when the terminal
    /// has no pixel layer. Pick one with [`Icons::spark`].
    pub spark: [&'static str; 8],
}

impl Icons {
    /// The set every terminal can draw — and the one the goldens are
    /// rasterized from.
    pub const UNICODE: Self = Self {
        pending: "○",
        running: "●",
        done: "✓",
        failed: "✗",
        waiting: "◌",
        blocked: "⊘",
        skipped: "↷",
        stopped: "■",
        active: "▸",
        planned: "·",
        collapsed: "▸",
        expanded: "▾",
        latest: "↓",
        bullet: "•",
        rail_tick: "╵",
        rail_break: "┆",
        warning: "⚠",
        blocker: "⊗",
        proposal: "⚑",
        signal: "◆",
        think: "✦",
        pinned: "★",
        checkpoint: "✱",
        brand: "▲",
        here: "▲",
        prompt: "›",
        keys: "⌨",
        undock: "⇱",
        collapse: "◂",
        file: "▤",
        edit: "✎",
        added: "+",
        removed: "−",
        spinner: Self::SPINNER,
        bar_full: "▰",
        bar_empty: "▱",
        spark: Self::SPARK,
    };

    /// The Nerd Font set: Font Awesome codepoints from the patched
    /// range, one cell wide like the rest.
    pub const NERD: Self = Self {
        pending: "\u{f10c}",   // circle-o
        running: "\u{f111}",   // circle
        done: "\u{f00c}",      // check
        failed: "\u{f00d}",    // times
        waiting: "\u{f110}",   // spinner
        blocked: "\u{f05e}",   // ban
        skipped: "\u{f01e}",   // repeat
        stopped: "\u{f04d}",   // stop
        active: "\u{f054}",    // chevron-right
        planned: "\u{f192}",   // dot-circle-o
        collapsed: "\u{f054}", // chevron-right
        expanded: "\u{f078}",  // chevron-down
        latest: "\u{f063}",    // arrow-down
        bullet: "•",
        rail_tick: "╵",
        rail_break: "┆",
        warning: "\u{f071}",    // exclamation-triangle
        blocker: "\u{f06a}",    // exclamation-circle
        proposal: "\u{f024}",   // flag
        signal: "\u{f0e7}",     // bolt
        think: "\u{f0eb}",      // lightbulb
        pinned: "\u{f005}",     // star
        checkpoint: "\u{f02e}", // bookmark
        brand: "\u{f135}",      // rocket
        here: "\u{f041}",       // map-marker
        prompt: "\u{f105}",     // angle-right
        keys: "\u{f11c}",       // keyboard
        undock: "\u{f08e}",     // external-link
        collapse: "\u{f053}",   // chevron-left
        file: "\u{f15b}",       // file
        edit: "\u{f044}",       // pencil-square
        added: "\u{f067}",      // plus
        removed: "\u{f068}",    // minus
        spinner: Self::SPINNER,
        bar_full: "▰",
        bar_empty: "▱",
        spark: Self::SPARK,
    };

    /// The spinner's rotation: one quadrant block orbiting the cell,
    /// a quarter turn per frame, clockwise from the top left.
    pub const SPINNER: [&'static str; 4] = ["▘", "▝", "▗", "▖"];

    /// The sparkline ramp: the eighth blocks, lowest to full.
    pub const SPARK: [&'static str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

    /// The glyphs the two typographic marks share, whatever the set:
    /// the list bullet, the progress bar's two cells, the spinner's four
    /// frames and the sparkline's eight heights.
    pub const SHARED: [&'static str; 17] = [
        "•", "╵", "┆", "▰", "▱", "▘", "▝", "▗", "▖", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█",
    ];

    /// Every glyph with the idea it stands for — the vocabulary as data,
    /// for auditing a set rather than reading it.
    #[must_use]
    pub const fn entries(&self) -> [(&'static str, &'static str); 47] {
        [
            ("pending", self.pending),
            ("running", self.running),
            ("done", self.done),
            ("failed", self.failed),
            ("waiting", self.waiting),
            ("blocked", self.blocked),
            ("skipped", self.skipped),
            ("stopped", self.stopped),
            ("active", self.active),
            ("planned", self.planned),
            ("collapsed", self.collapsed),
            ("expanded", self.expanded),
            ("latest", self.latest),
            ("bullet", self.bullet),
            ("rail_tick", self.rail_tick),
            ("rail_break", self.rail_break),
            ("warning", self.warning),
            ("blocker", self.blocker),
            ("proposal", self.proposal),
            ("signal", self.signal),
            ("think", self.think),
            ("pinned", self.pinned),
            ("checkpoint", self.checkpoint),
            ("brand", self.brand),
            ("here", self.here),
            ("prompt", self.prompt),
            ("keys", self.keys),
            ("undock", self.undock),
            ("collapse", self.collapse),
            ("file", self.file),
            ("edit", self.edit),
            ("added", self.added),
            ("removed", self.removed),
            ("spinner 0", self.spinner[0]),
            ("spinner 1", self.spinner[1]),
            ("spinner 2", self.spinner[2]),
            ("spinner 3", self.spinner[3]),
            ("bar_full", self.bar_full),
            ("bar_empty", self.bar_empty),
            ("spark 0", self.spark[0]),
            ("spark 1", self.spark[1]),
            ("spark 2", self.spark[2]),
            ("spark 3", self.spark[3]),
            ("spark 4", self.spark[4]),
            ("spark 5", self.spark[5]),
            ("spark 6", self.spark[6]),
            ("spark 7", self.spark[7]),
        ]
    }

    /// The glyph that spells `icon` in this set — the text form of the
    /// vector vocabulary, so a line built for a plain terminal and the
    /// mark drawn on a graphics one name the same idea.
    #[must_use]
    pub const fn of(&self, icon: Icon) -> &'static str {
        match icon {
            Icon::FoldClosed => self.collapsed,
            Icon::FoldOpen => self.expanded,
            Icon::Done => self.done,
            Icon::Failed => self.failed,
            Icon::Pending => self.pending,
            Icon::Ready | Icon::InProgress => self.active,
            Icon::Blocked => self.blocked,
            Icon::Pinned | Icon::Pin => self.pinned,
            Icon::Checkpoint => self.checkpoint,
            Icon::Signal => self.signal,
            Icon::Proposal => self.proposal,
            Icon::Here => self.here,
            Icon::Bullet => self.bullet,
            Icon::Warning => self.warning,
            Icon::Blocker => self.blocker,
            Icon::Stopped => self.stopped,
            Icon::Waiting => self.waiting,
            Icon::Skipped => self.skipped,
            Icon::Planned => self.planned,
            Icon::Collapse => self.collapse,
            Icon::Running => self.running,
            Icon::Spinner(step) => self.spin(step as u64),
            Icon::Undock => self.undock,
            Icon::File => self.file,
            Icon::Edit => self.edit,
            Icon::Keys => self.keys,
        }
    }

    /// The glyph for a check's outcome.
    #[must_use]
    pub const fn check(&self, passed: bool) -> &'static str {
        if passed { self.done } else { self.failed }
    }

    /// The glyph for a fold: open or shut.
    #[must_use]
    pub const fn fold(&self, expanded: bool) -> &'static str {
        if expanded {
            self.expanded
        } else {
            self.collapsed
        }
    }

    /// The spinner's frame at animation step `step` — the model's
    /// [`ticks`](crate::cockpit::Model::ticks), so the mark turns once
    /// per four steps and `view` needs no clock of its own.
    #[must_use]
    pub const fn spin(&self, step: u64) -> &'static str {
        self.spinner[(step % 4) as usize]
    }

    /// The sparkline cell for `unit` (`0..=1`, clamped): the lowest
    /// block at nothing, the full block at one. A non-finite value
    /// reads as nothing — a cell-drawn sparkline never blanks out.
    #[must_use]
    pub fn spark(&self, unit: f32) -> &'static str {
        let unit = if unit.is_finite() {
            unit.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let step = (unit * (self.spark.len() - 1) as f32).round() as usize;
        self.spark[step.min(self.spark.len() - 1)]
    }

    /// The glyph for a gate LED by its outcome (`None` while it runs).
    #[must_use]
    pub const fn gate(&self, passed: Option<bool>) -> &'static str {
        match passed {
            None => self.waiting,
            Some(true) => self.done,
            Some(false) => self.failed,
        }
    }
}

impl Default for Icons {
    fn default() -> Self {
        Self::UNICODE
    }
}

#[cfg(test)]
#[path = "icons.tests.rs"]
mod tests;
