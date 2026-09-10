use super::{EffectFuture, EffectRequest};

/// The session's shared permission decision path, supplied to a configured
/// effect adapter. Authorize the resolved request, then check cancellation
/// again immediately before admitting its effect.
pub trait EffectAuthorizer: Send + Sync {
    fn authorize(&self, request: EffectRequest) -> EffectFuture<'_, ()>;
    fn is_cancelled(&self) -> bool;
}
