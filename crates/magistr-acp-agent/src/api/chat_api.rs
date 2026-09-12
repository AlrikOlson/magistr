//! What differs between the three chat APIs.

use crate::{AgentError, ChatMessage, ModelSpec, StreamEvent};

/// One HTTP chat API, as [`ApiProvider`](crate::ApiProvider) drives it: the
/// registry of models, the request for one turn, and the decoding of one
/// line of the streamed body. Everything else — history, the stream loop,
/// usage, cancel — is the provider's.
pub trait ChatApi: Send + Sync + 'static {
    /// The provider's name on the command line.
    fn name(&self) -> &'static str;

    /// The models offered; the first is the default. Never empty once the
    /// API is constructed — a registry with nothing on it is a construction
    /// error, not an empty picker.
    fn models(&self) -> &[ModelSpec];

    /// The streaming request for one turn on `model` with the whole
    /// `history` (the last message is the prompt).
    fn request(
        &self,
        http: &reqwest::Client,
        model: &str,
        history: &[ChatMessage],
    ) -> reqwest::RequestBuilder;

    /// Whether a separate end sentinel must follow the generation finish.
    fn requires_done(&self) -> bool {
        false
    }

    /// Decode one line of the response body (without its line ending).
    /// Emit an explicit `Finish` for terminal evidence; EOF is never a finish.
    /// Lines that carry nothing decode to an empty list.
    fn decode(&self, line: &str) -> Result<Vec<StreamEvent>, AgentError>;
}
