//! The sixteen motions the design names, and no others.

use crate::theme::ThemeMode;

/// A named cockpit animation. Every transient identifies a motion so the
/// runtime can determine whether to keep ticking and which animation leads.
/// See `docs/architecture/desktop.md#motion` for the design rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MotionName {
    /// Prose is streaming: a band crosses the line.
    Sweep,
    /// Something new landed: a 220 ms rise, with a glow at night.
    Arrive,
    /// The working thing: the halo swells; by day the compass ring
    /// pulses.
    Breathe,
    /// Shipped history, alive: a slow phase per star — night only,
    /// ink is still.
    Twinkle,
    /// Claimable now: an expanding ring.
    ReadyPulse,
    /// A chunk ships: the workstream's hue flaring and settling to
    /// green; by day the mark inked in.
    Ignite,
    /// The unlock: light travels the edge; by day the pen strokes it.
    EdgeRun,
    /// Dashed → ready, in one step.
    Wake,
    /// The offer and the tally: the toast rises and fades, the counter
    /// ticks.
    ToastCounter,
    /// Cards dealt, staggered, worst first.
    Rise,
    /// The charting: stars appear in reasoning order.
    Born,
    /// The survey runs: a wedge circles.
    Radar,
    /// Input waits here: 1.1 s steps.
    Caret,
    /// Rest ↔ intent: 150 ms both ways.
    ChromeFade,
    /// Content loading: shimmer rows, ghost stars pulsing.
    Skeleton,
    /// Panning the sky: the stars lag the graticule.
    Parallax,
}

impl MotionName {
    /// Every name, in section III's order — the order a test walks
    /// and [`Model::motions_in_flight`](super::Model::motions_in_flight)
    /// answers in.
    pub const ALL: [Self; 16] = [
        Self::Sweep,
        Self::Arrive,
        Self::Breathe,
        Self::Twinkle,
        Self::ReadyPulse,
        Self::Ignite,
        Self::EdgeRun,
        Self::Wake,
        Self::ToastCounter,
        Self::Rise,
        Self::Born,
        Self::Radar,
        Self::Caret,
        Self::ChromeFade,
        Self::Skeleton,
        Self::Parallax,
    ];

    /// The motion's display name.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Sweep => "sweep",
            Self::Arrive => "arrive",
            Self::Breathe => "breathe",
            Self::Twinkle => "twinkle",
            Self::ReadyPulse => "ready-pulse",
            Self::Ignite => "ignite",
            Self::EdgeRun => "edge-run",
            Self::Wake => "wake",
            Self::ToastCounter => "toast · counter",
            Self::Rise => "rise",
            Self::Born => "born",
            Self::Radar => "radar",
            Self::Caret => "caret · blink",
            Self::ChromeFade => "chrome fade",
            Self::Skeleton => "skeleton",
            Self::Parallax => "parallax",
        }
    }

    /// What the motion means — a description of its effect.
    #[must_use]
    pub const fn means(self) -> &'static str {
        match self {
            Self::Sweep => "prose is streaming",
            Self::Arrive => "something new landed",
            Self::Breathe => "the working thing",
            Self::Twinkle => "shipped history, alive",
            Self::ReadyPulse => "claimable now",
            Self::Ignite => "a chunk ships",
            Self::EdgeRun => "the unlock",
            Self::Wake => "dashed → ready",
            Self::ToastCounter => "the offer · the tally",
            Self::Rise => "cards dealt",
            Self::Born => "the charting",
            Self::Radar => "the survey runs",
            Self::Caret => "input waits here",
            Self::ChromeFade => "rest ↔ intent",
            Self::Skeleton => "content loading",
            Self::Parallax => "panning the sky",
        }
    }

    /// How the motion renders at night, where the medium is light —
    /// the light and dark theme treatments.
    #[must_use]
    pub const fn by_night(self) -> &'static str {
        match self {
            Self::Sweep => "bright band crosses the line",
            Self::Arrive => "220 ms rise + glow",
            Self::Breathe => "halo swells",
            Self::Twinkle => "slow phase per star",
            Self::ReadyPulse => "expanding ring",
            Self::Ignite => "workstream-hue flare → green",
            Self::EdgeRun => "light travels the edge",
            Self::Wake => "steps once",
            Self::ToastCounter => "rise/fade · tick",
            Self::Rise => "staggered, worst first",
            Self::Born => "stars appear in reasoning order",
            Self::Radar => "light wedge circles",
            Self::Caret => "1.1 s steps",
            Self::ChromeFade => "150 ms both ways",
            Self::Skeleton => "shimmer rows · ghost stars pulse",
            Self::Parallax => "stars lag the graticule",
        }
    }

    /// How the motion renders by day, where the medium is ink — the
    /// light theme. `None` is the column's **none**:
    /// the twinkle has no day form, because ink is still.
    #[must_use]
    pub const fn by_day(self) -> Option<&'static str> {
        match self {
            Self::Sweep => Some("subtle band, no white"),
            Self::Arrive => Some("rise only"),
            Self::Breathe => Some("compass ring pulses"),
            Self::Twinkle => None,
            Self::ReadyPulse => Some("expanding ring, drawn"),
            Self::Ignite => Some("mark inked in → green"),
            Self::EdgeRun => Some("pen stroke draws the edge"),
            Self::Wake => Some("steps once"),
            Self::ToastCounter => Some("rise/fade · tick"),
            Self::Rise => Some("staggered, worst first"),
            Self::Born => Some("marks drawn in order"),
            Self::Radar => Some("graphite arc circles"),
            Self::Caret => Some("1.1 s steps"),
            Self::ChromeFade => Some("150 ms both ways"),
            Self::Skeleton => Some("shimmer rows · ghost stars pulse, no glow"),
            Self::Parallax => Some("marks lag the grid"),
        }
    }

    /// The column for `mode`'s medium — Mocha is the night, Latte the
    /// day — or `None` where the motion does not exist in that medium.
    #[must_use]
    pub const fn by(self, mode: ThemeMode) -> Option<&'static str> {
        match mode {
            ThemeMode::Dark => Some(self.by_night()),
            ThemeMode::Light => self.by_day(),
        }
    }

    /// Whether the window renders the motion as the two
    /// columns say. The five that answer `false` are not fully
    /// implemented: the streaming band, the wake, the toast's rise and
    /// fade (the counter's tick is shipped), the caret's blink and the
    /// skeleton rows (the ghost stars are shipped).
    #[must_use]
    pub const fn is_shipped(self) -> bool {
        !matches!(
            self,
            Self::Sweep | Self::Wake | Self::ToastCounter | Self::Caret | Self::Skeleton
        )
    }

    /// Whether the motion is texture rather than news: a periodic
    /// motion that holds its phase on a still cockpit and never asks
    /// for a tick of its own — the breath, the pulse, the twinkle, the
    /// ghost stars, the caret. Everything else is a bounded transient
    /// that *is* the news.
    #[must_use]
    pub const fn is_ambient(self) -> bool {
        matches!(
            self,
            Self::Breathe | Self::ReadyPulse | Self::Twinkle | Self::Skeleton | Self::Caret
        )
    }

    /// Where the motion ranks when a screen shows several at once:
    /// the smaller leads. News outranks texture, and among the news
    /// the ship moment outranks everything — it is the one moment the
    /// product is built around.
    #[must_use]
    pub const fn precedence(self) -> u8 {
        match self {
            Self::Ignite => 0,
            Self::EdgeRun => 1,
            Self::Born => 2,
            Self::Radar => 3,
            Self::Rise => 4,
            Self::Sweep => 5,
            Self::Arrive => 6,
            Self::Wake => 7,
            Self::ToastCounter => 8,
            Self::ChromeFade => 9,
            Self::Parallax => 10,
            Self::Breathe => 11,
            Self::ReadyPulse => 12,
            Self::Twinkle => 13,
            Self::Skeleton => 14,
            Self::Caret => 15,
        }
    }

    /// The dominant motion among `in_flight`, or `None` for a still screen.
    #[must_use]
    pub fn hero(in_flight: impl IntoIterator<Item = Self>) -> Option<Self> {
        in_flight.into_iter().min_by_key(|name| name.precedence())
    }
}

#[cfg(test)]
#[path = "motion_name.tests.rs"]
mod tests;
