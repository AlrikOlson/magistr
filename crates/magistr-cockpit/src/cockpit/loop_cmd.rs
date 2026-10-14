//! What the user asks of the iteration loop.

/// A loop decision `update` answers a key, a click or a ⌘K row with, for
/// the loop driver — as [`Cmd::Loop`](super::Cmd::Loop) carries it.
/// Every decision takes effect at the next iteration boundary; the turn
/// in flight is not touched (ctrl-C cancels that).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopCmd {
    /// Stop before the next iteration.
    Pause,
    /// Go on.
    Resume,
    /// Run exactly one iteration, then stop at the boundary after it —
    /// from a paused loop the next one, from a running loop the one in
    /// flight.
    Step,
    /// Skip the next iteration.
    Skip,
    /// End the loop.
    Abort,
}
