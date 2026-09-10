use super::{
    EffectAuthorizer, EffectCapabilities, EffectError, EffectFuture, EffectRequest, EffectResponse,
};
use alloc::sync::Arc;

/// Configured application capability for workspace and process effects.
/// Implementations resolve resources and authorize before admission. Protocol
/// adapters cannot substitute an ambient filesystem or process fallback.
pub trait EffectPort: Send + Sync {
    fn capabilities(&self) -> EffectCapabilities;
    fn execute(
        &self,
        request: EffectRequest,
        authorizer: Arc<dyn EffectAuthorizer>,
    ) -> EffectFuture<'_, EffectResponse>;
    /// Host-initiated cancellation of an owned terminal.
    fn kill_terminal(&self, terminal_id: &str) -> Result<(), EffectError>;
    /// Release resources when the owning connection closes. Must be idempotent.
    fn close(&self);
}
