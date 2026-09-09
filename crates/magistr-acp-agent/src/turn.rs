//! The provider's handle on one prompt turn.

use agent_client_protocol::schema::v1::{
    ContentBlock, ContentChunk, PermissionOption, Plan, RequestPermissionOutcome,
    RequestPermissionRequest, SessionId, SessionNotification, SessionUpdate, TextContent, ToolCall,
    ToolCallUpdate, UsageUpdate,
};
use agent_client_protocol::{Client, ConnectionTo};
use tokio_util::sync::CancellationToken;

use crate::AgentError;

/// One `session/prompt`, as the provider drives it.
///
/// The turn carries what the client sent (the content blocks and the model the
/// session is on) and is the only way the provider reaches the wire: the
/// `message` / `thought` / `tool_call` / `usage` / `plan` family emits
/// `session/update` notifications, [`request_permission`](Self::request_permission)
/// relays a permission request to the client and waits for its answer, and
/// [`cancelled`](Self::cancelled) resolves when the client sent
/// `session/cancel`. Every method may be called from any task; nothing here
/// holds the connection's dispatch loop.
pub struct Turn {
    session_id: SessionId,
    prompt: Vec<ContentBlock>,
    model: Option<String>,
    cancel: CancellationToken,
    cx: ConnectionTo<Client>,
}

impl Turn {
    pub(crate) fn new(
        session_id: SessionId,
        prompt: Vec<ContentBlock>,
        model: Option<String>,
        cancel: CancellationToken,
        cx: ConnectionTo<Client>,
    ) -> Self {
        Self {
            session_id,
            prompt,
            model,
            cancel,
            cx,
        }
    }

    /// The session this turn belongs to.
    #[must_use]
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    /// The content blocks the client sent.
    #[must_use]
    pub fn prompt(&self) -> &[ContentBlock] {
        &self.prompt
    }

    /// The text of the prompt: every text block, in order, joined by newlines.
    /// Non-text blocks (resources, images) are skipped.
    #[must_use]
    pub fn text(&self) -> String {
        self.prompt
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text(text) => Some(text.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The model the session is on — the provider's default, or whatever the
    /// client last selected through `session/set_config_option`. `None` when
    /// the provider offers no models.
    #[must_use]
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// Whether the client has cancelled this turn.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    /// Resolves once the client cancels this turn. Race the model against it.
    pub async fn cancelled(&self) {
        self.cancel.cancelled().await;
    }

    /// The turn's cancellation, for a provider that needs to hand it to
    /// something it does not poll itself (a subprocess, a request future).
    #[must_use]
    pub fn cancellation(&self) -> CancellationToken {
        self.cancel.clone()
    }

    /// Emit any `session/update` on this session.
    pub fn update(&self, update: SessionUpdate) -> Result<(), AgentError> {
        self.cx
            .send_notification(SessionNotification::new(self.session_id.clone(), update))
            .map_err(AgentError::from)
    }

    /// Stream a chunk of the model's reply (`agent_message_chunk`).
    pub fn message(&self, text: impl Into<String>) -> Result<(), AgentError> {
        self.update(SessionUpdate::AgentMessageChunk(text_chunk(text)))
    }

    /// Stream a chunk of the model's reasoning (`agent_thought_chunk`).
    pub fn thought(&self, text: impl Into<String>) -> Result<(), AgentError> {
        self.update(SessionUpdate::AgentThoughtChunk(text_chunk(text)))
    }

    /// Announce a tool call (`tool_call`).
    pub fn tool_call(&self, call: ToolCall) -> Result<(), AgentError> {
        self.update(SessionUpdate::ToolCall(call))
    }

    /// Report progress on a tool call (`tool_call_update`).
    pub fn tool_call_update(&self, update: ToolCallUpdate) -> Result<(), AgentError> {
        self.update(SessionUpdate::ToolCallUpdate(update))
    }

    /// Report the context window and cost (`usage_update`).
    pub fn usage(&self, usage: UsageUpdate) -> Result<(), AgentError> {
        self.update(SessionUpdate::UsageUpdate(usage))
    }

    /// Publish (or replace) the plan for this turn (`plan`).
    pub fn plan(&self, plan: Plan) -> Result<(), AgentError> {
        self.update(SessionUpdate::Plan(plan))
    }

    /// Ask the client for permission to run a tool call and wait for the
    /// answer. A cancel that arrives while the question is open answers it
    /// [`RequestPermissionOutcome::Cancelled`] — the client is required to
    /// answer that way itself, and this does not depend on it doing so.
    pub async fn request_permission(
        &self,
        tool_call: ToolCallUpdate,
        options: Vec<PermissionOption>,
    ) -> Result<RequestPermissionOutcome, AgentError> {
        let sent = self.cx.send_request(RequestPermissionRequest::new(
            self.session_id.clone(),
            tool_call,
            options,
        ));
        tokio::select! {
            response = sent.block_task() => Ok(response?.outcome),
            () = self.cancel.cancelled() => Ok(RequestPermissionOutcome::Cancelled),
        }
    }
}

fn text_chunk(text: impl Into<String>) -> ContentChunk {
    ContentChunk::new(ContentBlock::Text(TextContent::new(text.into())))
}
