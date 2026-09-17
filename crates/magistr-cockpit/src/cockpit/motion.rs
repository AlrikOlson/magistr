//! Everything on the cockpit that moves, as state the tick advances.

use super::eased::Eased;
use super::motion_key::MotionKey;
use super::motion_name::MotionName;

/// The motion layer's state: the two progress fills that ease, and the
/// step each transient started at, so the move it began can be read off
/// the tick count. All of it is a pure function of
/// [`Model::ticks`](super::Model::ticks); `update` retargets it after
/// every fold by comparing the model before and after
/// ([`Motion::observe`]), so no producer or key handler has to know
/// motion exists.
///
/// The transients come in two families. A *transition* — a tab switch,
/// a dock-card arrival, a gate that settled, a task or a chunk that
/// completed — is one wave the [`Pen`](crate::theme::Pen) pulses or pops
/// through. An *arrival* — the turn rows and tool cards a fold added —
/// is a fade the same pen brings in from its dim end. Each stamp that
/// needs to say *which* items moved carries the index the new ones start
/// at, taken from the count [`MotionKey`] held before the fold; that is
/// what lets the whole layer stay `Copy` instead of growing a map.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Motion {
    rail_fill: Eased,
    roadmap_fill: Eased,
    tab_switched: Option<f32>,
    dock_arrived: Option<f32>,
    /// The step rows arrived at, and the first row that is new.
    rows_arrived: Option<(f32, usize)>,
    /// The step gates settled at, and the first LED that is newly settled.
    gates_settled: Option<(f32, usize)>,
    /// The step a plan task completed at, and the row it sits on.
    task_completed: Option<(f32, usize)>,
    /// The step a roadmap chunk completed at.
    chunk_completed: Option<f32>,
    /// The step the trace last grew at, and how many steps it held
    /// before — where the sweep starts from, so a step arriving on a
    /// long trace draws only itself in and a trace arriving whole draws
    /// the graph in from the left.
    trace_grew: Option<(f32, usize)>,
    /// The step the header's activity ribbon last grew at, and how many
    /// of the slot's columns were already drawn — where its sweep starts
    /// from, exactly as `trace_grew` works for the trace graph.
    spark_grew: Option<(f32, usize)>,
    /// The step the pointer last moved to a different widget at, on a
    /// window — where the tooltip's dwell is counted from.
    hover_moved: Option<f32>,
    /// The step the rail last collapsed or expanded at, on a window —
    /// where its chrome fade starts.
    rail_toggled: Option<f32>,
}

impl Motion {
    /// Steps a transition — a tab switch, a dock arrival, a gate, task
    /// or chunk finishing — is shown for: about 240 ms at the runtime's
    /// tick.
    pub const TRANSITION_STEPS: u64 = 4;

    /// Steps an arriving row fades in over: about 180 ms at the
    /// runtime's tick, the short end Folmer's restraint asks for.
    pub const ARRIVAL_STEPS: u64 = 3;

    /// Steps the trace graph's sweep draws itself in over: about 360 ms
    /// at the runtime's tick. Longer than a row's fade, because a sweep
    /// has a distance to cover and a snapped-out graph reads as a
    /// glitch; no longer than a glide, so it is over inside the same
    /// settling window every other transient is.
    pub const SWEEP_STEPS: u64 = 6;

    /// Steps the active chunk's glow takes to breathe in and out once:
    /// about 1.4 s at the runtime's tick, and — deliberately — exactly
    /// the settling window the screenshot tests drive a model through
    /// (`SETTLE_STEPS`, ), so every still the rasterizer pins is
    /// taken at the foot of the wave and no PNG golden moves.
    pub const GLOW_STEPS: u64 = 24;

    /// A calm, continuous return sweep over 1.92 seconds. The segment slows
    /// at each end without disappearing or jumping back to its start.
    #[must_use]
    pub fn indeterminate_position(now: f32) -> u16 {
        let phase = now.rem_euclid(32.0) / 32.0;
        ((1.0 - (phase * std::f32::consts::TAU).cos()) * 500.0).round() as u16
    }

    /// Steps the pointer must rest on one widget before its tooltip
    /// appears: about 480 ms at the runtime's tick.
    ///
    /// A dwell and not an instant reveal, because a pointer crossing a
    /// busy cockpit passes over a dozen hittables on its way somewhere
    /// and a card that appeared under each of them would be a strobe
    /// rather than help. Longer than every other transient here, since
    /// this one is timing a *human* rather than drawing one.
    pub const DWELL_STEPS: u64 = 8;

    /// Steps the rail's chrome fades in over when it collapses or
    /// expands: the design's *chrome fade* (rest ↔ intent, ), about 150 ms — three ticks, the arrival's own length,
    /// since a pane's rows appearing is an arrival of chrome.
    pub const CHROME_FADE_STEPS: u64 = 3;

    /// Steps of no input the sky waits before its chrome exhales:
    /// about three seconds at the runtime's tick, which is the
    /// "seconds of no input" asks for and long enough that
    /// a reader pausing over a star does not watch the room empty
    /// around them.
    ///
    /// Unlike every other constant here this one is not read through
    /// [`Motion`] at all: rest is timed from
    /// [`Model::ticks`](super::Model::ticks) against the tick the last
    /// input landed on, because the fold that would take a stamp never
    /// happens on the surface that needs one — a native still is
    /// assembled on a terminal model and only told it is a window when
    /// it is painted. It lives here so the whole motion vocabulary's
    /// timings stay in one file.
    pub const REST_STEPS: u64 = 50;

    /// Steps between two cards of the Brief's deal — the *rise*,
    /// 300 ms apart at the runtime's tick. Each card
    /// then fades in over [`Motion::ARRIVAL_STEPS`], so the deal reads
    /// as a hand laying cards down rather than a list appearing.
    pub const RISE_STAGGER: u64 = 5;

    /// Where the active chunk's glow is in its breath at step `now`,
    /// `0..1` around one raised cosine — the phase
    /// [`Pen::pulse`](crate::theme::Pen::pulse) and the tech-tree
    /// painter both take, so the cell node and the pixel halo agree.
    ///
    /// Unlike every other motion here this one has no stamp and no end:
    /// it is a periodic function of the tick and nothing else, and it
    /// is deliberately absent from [`Motion::is_settling`]. The glow
    /// never asks for a tick of its own — it breathes while the cockpit
    /// is already moving and holds its phase when it is not, which is
    /// what keeps an idle Roadmap tab at zero frames a second. At the
    /// foot of the wave (`now` a multiple of [`Motion::GLOW_STEPS`],
    /// which every settled still is) the glow matches the static frame.
    #[must_use]
    pub fn glow_phase(now: f32) -> f32 {
        now.rem_euclid(Self::GLOW_STEPS as f32) / Self::GLOW_STEPS as f32
    }

    /// Fold what changed between `before` and `after` at step `now`:
    /// the fills retarget (easing when `animate`, landing otherwise),
    /// a tab change or a longer dock start a transition, rows the fold
    /// added start a fade, and a gate, task or chunk that finished pops
    /// — all of it only when `animate`, since a transient with no tick
    /// behind it would never end.
    pub fn observe(&mut self, before: MotionKey, after: MotionKey, now: f32, animate: bool) {
        if after.rail.1 > 0 {
            let (done, total) = after.rail;
            self.rail_fill
                .retarget(done as f32 / total as f32, now, animate);
        }
        if after.roadmap.1 > 0 {
            let (done, total) = after.roadmap;
            self.roadmap_fill
                .retarget(done as f32 / total as f32, now, animate);
        }
        if !animate {
            return;
        }
        if before.tab != after.tab {
            self.tab_switched = Some(now);
        }
        if after.dock > before.dock {
            self.dock_arrived = Some(now);
        }
        // Several folds can land between two ticks — a burst of agent
        // deltas, two gates finishing at once. Merging them keeps the
        // whole run arriving together instead of only the last fold's
        // share of it.
        if after.rows > before.rows {
            self.rows_arrived = Some((now, Self::merge(self.rows_arrived, now, before.rows)));
        }
        if after.gates > before.gates {
            self.gates_settled = Some((now, Self::merge(self.gates_settled, now, before.gates)));
        }
        if after.tasks.0 > before.tasks.0 {
            self.task_completed = Some((now, after.tasks.1));
        }
        // The counter that moves is the rail's `done/total`, which is
        // also what the rail's bar fills to — so the bar glides and the
        // number pops on the same fact. A roadmap arriving is a seed,
        // not a completion: the first status the engine sends can carry
        // a hundred done chunks and the counter has nothing to pop
        // about. Once a total is known, every step of the count is a
        // chunk that finished — including the first one.
        if before.rail.1 > 0 && after.rail.0 > before.rail.0 {
            self.chunk_completed = Some(now);
        }
        if after.trace > before.trace {
            self.trace_grew = Some((now, Self::merge(self.trace_grew, now, before.trace)));
        }
        if after.activity > before.activity {
            self.spark_grew = Some((now, Self::merge(self.spark_grew, now, before.activity)));
        }
        // A hover that *changed* restarts the dwell; one that did not is
        // the pointer resting, which is exactly what the dwell measures.
        // On a terminal `hover` is always `None`, so this never fires.
        if before.hover != after.hover {
            self.hover_moved = after.hover.map(|_| now);
        }
        // The rail collapsing or expanding fades what it then shows in.
        // `None` on a terminal on both sides, so this never fires there.
        if animate && before.rail_open.is_some() && before.rail_open != after.rail_open {
            self.rail_toggled = Some(now);
        }
    }

    /// Where the rail's chrome fade is at step `now`, `0..1` — `None`
    /// once the rail has settled, or where it was never toggled.
    #[must_use]
    pub fn rail_fade(&self, now: f32) -> Option<f32> {
        Self::over(self.rail_toggled, now, Self::CHROME_FADE_STEPS)
    }

    /// The rail's progress fill at step `now`, `0..=1`.
    #[must_use]
    pub fn rail_fill(&self, now: f32) -> f32 {
        self.rail_fill.at(now)
    }

    /// The roadmap card's progress fill at step `now`, `0..=1`.
    #[must_use]
    pub fn roadmap_fill(&self, now: f32) -> f32 {
        self.roadmap_fill.at(now)
    }

    /// How far the tab switch's transition has come at step `now`,
    /// `0..1` while it runs — `None` once it is over (or never started).
    #[must_use]
    pub fn tab_transition(&self, now: f32) -> Option<f32> {
        Self::transition(self.tab_switched, now)
    }

    /// How far the dock arrival's transition has come at step `now`.
    #[must_use]
    pub fn dock_transition(&self, now: f32) -> Option<f32> {
        Self::transition(self.dock_arrived, now)
    }

    /// How far turn row `row`'s fade-in has come at step `now`, `0..1`
    /// from the dim pen — `None` for a row that was already there, and
    /// once the fade is over, so a settled row is exactly its own pen.
    #[must_use]
    pub fn row_arrival(&self, row: usize, now: f32) -> Option<f32> {
        let (since, first) = self.rows_arrived?;
        (row >= first).then(|| Self::over(Some(since), now, Self::ARRIVAL_STEPS))?
    }

    /// How far gate LED `index`'s pop has come at step `now` — `None`
    /// for an LED that had already settled, and once the pop is over.
    #[must_use]
    pub fn gate_pop(&self, index: usize, now: f32) -> Option<f32> {
        let (since, first) = self.gates_settled?;
        (index >= first).then(|| Self::transition(Some(since), now))?
    }

    /// How far the pop of the plan task on rail row `index` has come at
    /// step `now` — `None` for every other row, and once it is over.
    #[must_use]
    pub fn task_pop(&self, index: usize, now: f32) -> Option<f32> {
        let (since, at) = self.task_completed?;
        (index == at).then(|| Self::transition(Some(since), now))?
    }

    /// How far the rail's roadmap counter's pop has come at step `now`.
    #[must_use]
    pub fn chunk_pop(&self, now: f32) -> Option<f32> {
        Self::transition(self.chunk_completed, now)
    }

    /// How far the trace graph's sweep has come at step `now`, as the
    /// fraction of the graph's width that is revealed — `None` once it
    /// is over, and then the graph draws whole.
    ///
    /// The sweep starts at the share of `total` steps that were already
    /// on screen, so one step landing on a long trace draws in only its
    /// own end of the graph while a trace arriving whole draws in from
    /// the left. The share is approximate — the painter spreads its
    /// steps over the width with a little padding either side — which
    /// costs at most one step of slop at the start of a sweep and
    /// nothing at all at its end.
    #[must_use]
    pub fn trace_sweep(&self, total: usize, now: f32) -> Option<f32> {
        let (since, first) = self.trace_grew?;
        if total == 0 {
            return None;
        }
        let t = Self::over(Some(since), now, Self::SWEEP_STEPS)?;
        let from = first as f32 / total as f32;
        Some(from + (1.0 - from) * t)
    }

    /// How far the header's activity monitor's sweep has come at step
    /// `now`, as the fraction of the slot's `width` that is revealed —
    /// `None` once it is over, and then the line draws whole. The same
    /// curve as [`Motion::trace_sweep`], starting at the share of the
    /// slot that was already drawn, so the monitor arriving with a burst
    /// of busy frames behind it draws in from the left while one sample
    /// landing on a drawn ribbon draws in only its own column.
    ///
    /// The `width` cannot saturate the sweep on its own — that is
    /// [`MotionKey::activity`](super::MotionKey::activity)'s job. It
    /// carries the count *already capped at the slot's width*, so a
    /// streaming session, which lands a busy frame at a time for as long
    /// as the turn runs, stops changing the key once the ribbon fills
    /// the slot and takes no further stamp. Capping here instead would
    /// leave every busy frame re-arming [`Motion::is_settling`] for
    /// another [`Motion::SWEEP_STEPS`], holding the runtime's tick past
    /// the news for a sweep that starts at 1.0 and draws nothing.
    #[must_use]
    pub fn spark_sweep(&self, width: usize, now: f32) -> Option<f32> {
        let (since, first) = self.spark_grew?;
        if width == 0 {
            return None;
        }
        let t = Self::over(Some(since), now, Self::SWEEP_STEPS)?;
        let from = first as f32 / width as f32;
        Some(from + (1.0 - from) * t)
    }

    /// Whether the pointer has rested on one widget long enough for its
    /// tooltip, at step `now`.
    ///
    /// `false` until the dwell is up and `false` for a pointer that is
    /// over nothing, so the card appears once and then simply stays —
    /// which is why the dwell leaves [`Motion::is_settling`] the moment
    /// it elapses and an idle window goes back to zero frames a second.
    #[must_use]
    pub fn hover_dwelt(&self, now: f32) -> bool {
        self.hover_moved
            .is_some_and(|since| (now - since).max(0.0) >= Self::DWELL_STEPS as f32)
    }

    /// Motion names active at step `now`, in registry order:
    ///
    /// * *sweep* — the trace graph and the activity ribbon drawing
    ///   themselves in;
    /// * *arrive* — the turn rows and the dock card fading in;
    /// * *ignite* — a gate LED popping as it settles, the gates'
    ///   cascade that opens the ship moment;
    /// * *toast · counter* — the tally: the rail's and the roadmap's
    ///   fills gliding to their count, the counter and a plan row
    ///   popping on a completion;
    /// * *chrome fade* — the stage's tab pulsing across a switch, the
    ///   tooltip's dwell (intent chrome timing a human), the rail's
    ///   rows fading in on a collapse or expand.
    ///
    /// [`Motion::is_settling`] is derived from this and nothing else,
    /// so a transient with no name here cannot hold the tick — the
    /// vocabulary is the mechanism, not a label on it.
    pub fn in_flight(&self, now: f32) -> impl Iterator<Item = MotionName> {
        let over = |stamp: Option<(f32, usize)>, steps: u64| {
            stamp.is_some_and(|(since, _)| Self::over(Some(since), now, steps).is_some())
        };
        let sweep =
            over(self.trace_grew, Self::SWEEP_STEPS) || over(self.spark_grew, Self::SWEEP_STEPS);
        let arrive =
            self.dock_transition(now).is_some() || over(self.rows_arrived, Self::ARRIVAL_STEPS);
        let ignite = over(self.gates_settled, Self::TRANSITION_STEPS);
        let counter = self.rail_fill.is_settling(now)
            || self.roadmap_fill.is_settling(now)
            || self.chunk_pop(now).is_some()
            || over(self.task_completed, Self::TRANSITION_STEPS);
        // The dwell is a transient like the rest: the tick runs for
        // the eight steps it takes and stops, so a window with the
        // pointer parked on a chip costs nothing once the card is up.
        let chrome = self.tab_transition(now).is_some()
            || Self::over(self.hover_moved, now, Self::DWELL_STEPS).is_some()
            || self.rail_fade(now).is_some();
        [
            (MotionName::Sweep, sweep),
            (MotionName::Arrive, arrive),
            (MotionName::Ignite, ignite),
            (MotionName::ToastCounter, counter),
            (MotionName::ChromeFade, chrome),
        ]
        .into_iter()
        .filter_map(|(name, moving)| moving.then_some(name))
    }

    /// Whether anything here still moves at step `now`, so the runtime
    /// should keep its tick running — exactly when
    /// [`Motion::in_flight`] names something.
    #[must_use]
    pub fn is_settling(&self, now: f32) -> bool {
        self.in_flight(now).next().is_some()
    }

    fn transition(since: Option<f32>, now: f32) -> Option<f32> {
        Self::over(since, now, Self::TRANSITION_STEPS)
    }

    /// The index a stamp taken at step `now` starts from, given `first`
    /// from this fold: the lower of the two when a stamp was already
    /// taken on this very step, so folds sharing a tick share one move.
    fn merge(stamp: Option<(f32, usize)>, now: f32, first: usize) -> usize {
        match stamp {
            // The same *tick*, not the same instant: on a window two
            // folds land at two phases of one tick and are still the
            // burst this merges. Taking the newer stamp without merging
            // would drop the rows already fading out of the range the
            // stamp names, snapping them to settled mid-fade.
            Some((since, at)) if since.floor() == now.floor() => at.min(first),
            _ => first,
        }
    }

    /// How far a transient stamped at `since` has come at step `now`,
    /// `0..1` over `steps` — `None` once it is over, or never started.
    fn over(since: Option<f32>, now: f32, steps: u64) -> Option<f32> {
        let elapsed = (now - since?).max(0.0);
        (elapsed < steps as f32).then(|| elapsed / steps as f32)
    }
}

#[cfg(test)]
#[path = "motion.tests.rs"]
mod tests;
