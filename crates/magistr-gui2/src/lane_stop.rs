/// One stop on the Trace view's branch graph — a step the lane shows,
/// as [`TraceGraph`](crate::trace_graph::TraceGraph) draws it and the
/// composition derives it from the cockpit's [`Trace`].
///
/// The stop carries the step's **feed index** and not the step itself:
/// the drawing registers that index as the step's hit target
/// (`Widget::trace_row`), so a click on a lane dot is the same click
/// as one on the card — the cockpit's one toggle — and a re-seeded
/// trace cannot leave a stale number behind.
///
/// [`Trace`]: magistr_cockpit::cockpit::Trace
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LaneStop {
    /// The step's index in the trace's feed.
    pub index: usize,
    /// The step is the one the reader expanded: it wears the accent and
    /// its ring.
    pub focused: bool,
    /// The step is still being written — the trace's newest, not yet
    /// final: a dashed ring rather than a dot.
    pub open: bool,
    /// A branch forks off this step: the feed index of the branch's
    /// first step, which the fork's tip answers a click for.
    pub fork: Option<usize>,
}
