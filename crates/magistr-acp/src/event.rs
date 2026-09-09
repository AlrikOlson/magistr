//! Typed events the transport emits to its consumer.

use agent_client_protocol::schema::v1::{
    AgentCapabilities, Meta, PromptResponse, SessionConfigOption, SessionUpdate,
};

/// One event from the agent, already parsed into `agent-client-protocol`
/// types so consumers convert straight into their own message model.
#[derive(Debug, Clone)]
pub enum AcpEvent {
    /// A `session/update` notification: message chunk, tool call, plan, usage…
    SessionUpdate {
        /// The update payload (boxed: `SessionUpdate` is ~500 bytes and would
        /// otherwise size every event).
        update: Box<SessionUpdate>,
        /// Session the update belongs to.
        session_id: String,
    },
    /// A turn finished — the `session/prompt` response with its stop reason.
    PromptResponse {
        /// The response payload.
        response: PromptResponse,
        /// Session the response belongs to.
        session_id: String,
    },
    /// The handshake completed: capabilities from `initialize`, the session
    /// id and config options from `session/new` or `session/load`.
    Initialized {
        /// Negotiated agent capabilities (boxed: it is by far the largest
        /// payload, and every other event would pay for it otherwise).
        agent_capabilities: Box<AgentCapabilities>,
        /// Session id the agent assigned (or the one that was loaded).
        session_id: String,
        /// Config options (model, mode, thought level) the agent advertises.
        config_options: Option<Vec<SessionConfigOption>>,
        /// The `_meta` of the `initialize` response — where an agent
        /// advertises its vendor extensions (the retired npm shim's `goal`,
        /// `steering`, …). The transport carries it whole and reads
        /// nothing out of it; a consumer parses the keys it knows.
        agent_meta: Option<Meta>,
    },
    /// A chunk of a terminal's raw output — the bytes the agent's command
    /// wrote, escapes and all, in the order it wrote them — for a live
    /// pane. The agent itself reads the stripped text through
    /// `terminal/output`; this is the cockpit's copy.
    Terminal {
        /// The terminal the bytes came from, as `terminal/create` named it.
        terminal_id: String,
        /// The bytes as written.
        bytes: Vec<u8>,
    },
    /// Something the transport itself wants the UI to know about — a
    /// permission request, a file read, a terminal spawn, a warning.
    System {
        /// Event subtype (`permission_request`, `fs_read`, `terminal_create`, …).
        subtype: String,
        /// Opaque payload.
        data: serde_json::Value,
    },
}
