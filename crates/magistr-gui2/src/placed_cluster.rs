use magistr_cockpit::cockpit::StarState;
use magistr_cockpit::layout::PxRect;

/// One project's cluster laid on the Galaxy's field, in the field's own
/// logical pixels: where each of its stars sits and what state each is
/// in, the chain joining them, where its label starts, and the box a
/// click on it answers in.
///
/// Answered by [`GalaxyLayout`](crate::galaxy_layout::GalaxyLayout) and
/// read by two consumers that must agree to the pixel — the
/// composition, which pins the label, and the drawing, which puts down
/// the stars — so neither derives a position of its own.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedCluster {
    /// Each star's centre.
    pub stars: Vec<(f32, f32)>,
    /// Each star's state, in the same order: shipped, working or
    /// asking.
    pub states: Vec<StarState>,
    /// The chain: pairs of star indices an edge joins.
    pub edges: Vec<(usize, usize)>,
    /// The top-left corner of the label block under the cluster.
    pub label: (f32, f32),
    /// The box the cluster answers a click in — its stars and its
    /// label together.
    pub bounds: PxRect,
}
