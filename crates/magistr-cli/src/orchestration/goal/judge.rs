//! The judge, as a port.

use std::future::Future;
use std::pin::Pin;

use super::judge_input::JudgeInput;
use super::verdict::Verdict;

/// Something that answers a [`JudgeInput`] with a [`Verdict`] — a fresh
/// model behind a transport (goal-judge-session), or a scripted answer
/// in the driver's tests.
///
/// A judge is *asked*, never trusted: whatever it answers goes through
/// [`Decision::reconcile`](super::Decision::reconcile) against the real
/// gates before the loop acts. It cannot fail — a transport that cannot
/// reach its model answers [`Verdict::not_yet`] with the reason, so the
/// loop iterates rather than wedges.
pub trait Judge: Send + Sync {
    /// The verdict for `input`.
    fn judge<'a>(
        &'a self,
        input: &'a JudgeInput,
    ) -> Pin<Box<dyn Future<Output = Verdict> + Send + 'a>>;
}
