//! What one line of a streamed reply decodes to.

/// An event decoded from the response body. A line may decode to several
/// (a chunk carrying both text and usage) or to none (a comment, a
/// keep-alive, an SSE `event:` field).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    /// A piece of the model's reply, streamed to the client as it arrives.
    Text(String),
    /// The turn's token counts. When an API reports them on every chunk the
    /// last report is the one that counts.
    Usage {
        /// Prompt tokens — the whole history, since every request carries it.
        input: u64,
        /// Completion tokens.
        output: u64,
    },
}
