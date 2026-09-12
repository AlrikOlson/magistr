use super::{EffectFuture, SourceWriteReceipt};

/// The route Magistr's own source writes take once the session has
/// authorized them: the admitted workspace's transaction pipeline, which
/// journals the intent and answers a truthful receipt. The host composes
/// one only when it admitted the workspace itself; a port without one
/// writes directly and says so. Authorization always precedes the route —
/// an implementation is never asked to write what the session refused.
pub trait SourceWriter: Send + Sync {
    /// Write `content` as the whole content of the file at the absolute,
    /// already resolved `path` inside the workspace.
    fn write_text(&self, path: &str, content: &str) -> EffectFuture<'_, SourceWriteReceipt>;
}
