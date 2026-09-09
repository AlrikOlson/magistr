//! The ship moment in flight.

/// A ship moment as the sky draws it: the star that
/// just shipped, the hue it ignites in, the stars its completion made
/// ready, and how far each of the two motions has come.
///
/// # The two motions are sequenced, not concurrent
///
/// *Ignite* runs first and *edge-run* only after it, because the
/// picture is a consequence and not a decoration: the light leaves the
/// star that caused the unlock, so it cannot leave before that star has
/// lit. [`ShipRun::edge_run`] is therefore `0.0` for the whole of the
/// ignite, which is exactly the frame in which no light has travelled
/// any of the edge yet.
///
/// # Why it goes out of existence
///
/// [`ShipRun::at`] answers `None` once both motions are over rather
/// than pinning them at `1.0`. A settled sky must draw the same
/// instances as a sky that never shipped anything — otherwise every
/// still after a completion would carry a flare nobody asked for, and
/// the "an idle cockpit paints nothing" invariant would be a comment
/// rather than a property. Reduced motion never records a run at all
/// (see [`Roadmap::apply`](super::Roadmap::apply)), so its end frame is
/// structural for the same reason.
#[derive(Clone, Debug, PartialEq)]
pub struct ShipRun {
    /// The shipped star, as a graph index —
    /// [`Roadmap::dag_nodes`](super::Roadmap::dag_nodes)' own index
    /// space.
    pub star: usize,
    /// Its workstream's ordinal — [`DagNode::region`](super::DagNode),
    /// which is the roadmap's own `group` field banded, so the hue
    /// needs no taxonomy of its own.
    pub region: usize,
    /// The stars the completion made ready, as graph indices. Exactly
    /// the chunks whose every dependency has now shipped and which had
    /// not been ready before — the edge-run's targets, and nothing
    /// else.
    pub unlocked: Vec<usize>,
    /// How far the ignite has come, `0..=1`: `0` is the flare at the
    /// workstream's full hue and `1` is the star settled to
    /// shipped-green.
    pub ignite: f32,
    /// How far the light has run the edges, `0..=1` — `0` until the
    /// ignite is over.
    pub edge_run: f32,
}

impl ShipRun {
    /// Steps the ignite takes: about 360 ms at the runtime's tick.
    ///
    /// Longer than a transition's pop, because this is the one moment
    /// the whole product is built around and a flare nobody sees is not
    /// a celebration; short enough that the edge-run behind it still
    /// lands inside one settling window.
    pub const IGNITE_STEPS: u64 = 6;

    /// Steps the light takes to run an edge: about 720 ms. Twice the
    /// ignite, because it has a distance to cover and the eye has to be
    /// able to follow it to the far end — the far end is the news.
    pub const EDGE_RUN_STEPS: u64 = 12;

    /// The whole moment's length.
    pub const STEPS: u64 = Self::IGNITE_STEPS + Self::EDGE_RUN_STEPS;

    /// Whether a completion at step `since` is still showing anything
    /// at step `now` — [`ShipRun::at`]'s window on its own, for the
    /// callers that only need to know whether to keep ticking.
    #[must_use]
    pub fn is_live(since: f32, now: f32) -> bool {
        (0.0..Self::STEPS as f32).contains(&(now - since))
    }

    /// The run a completion at step `since` is showing at step `now`,
    /// or `None` once it is over (or before it began, which a tick
    /// count that ran backwards would be).
    #[must_use]
    pub fn at(
        star: usize,
        region: usize,
        unlocked: Vec<usize>,
        since: f32,
        now: f32,
    ) -> Option<Self> {
        if !Self::is_live(since, now) {
            return None;
        }
        let elapsed = now - since;
        let ignite = (elapsed / Self::IGNITE_STEPS as f32).clamp(0.0, 1.0);
        let edge_run =
            ((elapsed - Self::IGNITE_STEPS as f32) / Self::EDGE_RUN_STEPS as f32).clamp(0.0, 1.0);
        Some(Self {
            star,
            region,
            unlocked,
            ignite,
            edge_run,
        })
    }

    /// Whether the light runs to `star` — true only for a star this
    /// completion made ready, which is what tells a newly-ready star
    /// apart from one that was already ready when the moment began.
    /// How far the light has come to it is [`ShipRun::edge_run`], the
    /// same fraction for every target: they all leave the same star at
    /// the same instant.
    #[must_use]
    pub fn runs_to(&self, star: usize) -> bool {
        self.unlocked.contains(&star)
    }
}

#[cfg(test)]
#[path = "ship_run.tests.rs"]
mod tests;
