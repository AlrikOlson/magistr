//! Guarantees implemented by a provider adapter, independent of its model catalog.

use serde::Serialize;

/// Conservative session guarantees. False means callers cannot rely on the
/// operation, even when the underlying vendor offers a similar feature.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct ProviderCapabilities {
    /// Existing history can be restored after the adapter process exits.
    pub durable_load: bool,
    /// A successful load replays the persisted conversation before responding.
    pub history_replay: bool,
    /// Later prompts continue the open session in this process.
    pub ephemeral_continuation: bool,
    /// The direct provider library supports steering; ACP v1 does not expose it.
    pub library_steering: bool,
    /// Model selection applies before the next turn.
    pub model_change: bool,
    /// Session startup accepts an explicit reasoning effort.
    pub effort_selection: bool,
    /// Cancellation waits for a confirmed terminal runtime event.
    pub terminal_cancellation_ack: bool,
    /// The adapter runs tools and relays their permission requests.
    pub tools: bool,
    /// Embedded resources are consumed as prompt context.
    pub embedded_context: bool,
    /// Image prompt blocks are consumed.
    pub image: bool,
    /// Audio prompt blocks are consumed.
    pub audio: bool,
}

impl ProviderCapabilities {
    /// An in-memory text conversation with model selection and no tool harness.
    #[must_use]
    pub const fn text_session() -> Self {
        Self {
            durable_load: false,
            history_replay: false,
            ephemeral_continuation: true,
            library_steering: false,
            model_change: true,
            effort_selection: false,
            terminal_cancellation_ack: false,
            tools: false,
            embedded_context: false,
            image: false,
            audio: false,
        }
    }
}
