//! Caps the MCP protocol version `magistr mcp-serve` will negotiate.
//!
//! The Claude Agent SDK's bundled CLI (spawned by the retired npm shim,
//! i.e. every `magistr next` iteration at the time) requested protocol
//! 2026-07-28 and then rejected the SEP-2322 `resultType` discriminator rmcp
//! stamps on that era's results — `tools/list` failed its validation, the
//! client retried three times, gave up and killed the server, so the session
//! had no magistr tools. Standalone Claude Code negotiates the pre-2026 era,
//! where rmcp strips `resultType`, and works — and it is what the in-binary
//! `claude` provider drives now (the recorded mcp_connected fixture in
//! crates/magistr-acp-agent shows `magistr mcp-serve` connected under
//! 2.1.258). The cap stays: it costs nothing and holds for every client.
//!
//! rmcp negotiates by echoing any *known* client-requested version
//! (`negotiate_protocol_version`), so a `ServerHandler::initialize` override
//! cannot lower it. The one hook a server author has is the transport: this
//! wrapper rewrites the requested version before rmcp sees it, and rmcp then
//! negotiates — and shapes every result for — the capped era.
//!
//! The cap also answers a `server/discover` that arrives before `initialize`.
//! Claude Code opens that way, is offered only the capped eras, and falls
//! back to `initialize` and legacy requests. rmcp holds a session whose first
//! message was not `initialize` to the 2026-07-28 inline lifecycle for good:
//! every later request without per-request `_meta` is refused, `tools/list`
//! included, and the session has no magistr tools. Answered here, the
//! discover never reaches rmcp, which then opens the legacy session on
//! `initialize` as it does for a client that never discovered.

use rmcp::ServerHandler;
use rmcp::model::{
    ClientJsonRpcMessage, ClientRequest, DiscoverResult, GetMeta, ProtocolVersion,
    ServerJsonRpcMessage, ServerResult,
};
use rmcp::service::{RoleServer, TxJsonRpcMessage};
use rmcp::transport::Transport;

/// The era served in place of 2026-07-28+: the newest version whose result
/// shapes carry no `resultType`, and the one the proven-good standalone
/// Claude Code pairing negotiates.
const SERVED: ProtocolVersion = ProtocolVersion::V_2025_06_18;

/// The version a client's request is admitted as: itself below 2026-07-28,
/// the served era from there on. The one rule behind [`ProtocolCap`], also
/// applied by the workspace bridge when it answers `initialize` itself.
pub(crate) fn capped(requested: ProtocolVersion) -> ProtocolVersion {
    if requested.as_str() >= ProtocolVersion::V_2026_07_28.as_str() {
        SERVED
    } else {
        requested
    }
}

/// Clamp `message`'s requested MCP protocol version below 2026-07-28: the
/// `initialize` params, and the per-request `_meta` version that can re-raise
/// the era on any later request.
fn clamp(message: &mut ClientJsonRpcMessage) {
    let ClientJsonRpcMessage::Request(request) = message else {
        return;
    };
    if let ClientRequest::InitializeRequest(init) = &mut request.request {
        init.params.protocol_version = capped(init.params.protocol_version.clone());
    }
    let meta = request.request.get_meta_mut();
    if let Some(version) = meta.protocol_version()
        && capped(version.clone()) != version
    {
        meta.set_protocol_version(SERVED);
    }
}

/// The opening of one session: whether the server has been given its first
/// request yet, and the discovery answer served until then.
struct Opening {
    discovery: DiscoverResult,
    is_open: bool,
}

impl Opening {
    const fn new(discovery: DiscoverResult) -> Self {
        Self {
            discovery,
            is_open: false,
        }
    }

    /// The cap's own answer to `message`, when it is a well-formed
    /// `server/discover` ahead of the session's first request. Any other
    /// request opens the session, and everything from there on — a later
    /// discover included — is the server's to answer.
    fn reply_to(&mut self, message: &ClientJsonRpcMessage) -> Option<ServerJsonRpcMessage> {
        let ClientJsonRpcMessage::Request(request) = message else {
            return None;
        };
        if self.is_open {
            return None;
        }
        let is_discover = matches!(&request.request, ClientRequest::DiscoverRequest(_))
            && request
                .request
                .get_meta()
                .missing_required_keys(&ProtocolVersion::V_2026_07_28)
                .is_empty();
        if !is_discover {
            self.is_open = true;
            return None;
        }
        Some(ServerJsonRpcMessage::response(
            ServerResult::DiscoverResult(self.discovery.clone()),
            request.id.clone(),
        ))
    }
}

/// A [`Transport`] wrapper that applies [`clamp`] to every inbound message,
/// pinning the session to a protocol era every Claude client accepts, and
/// answers the discovery that precedes `initialize`.
pub struct ProtocolCap<T> {
    inner: T,
    opening: Opening,
}

impl<T> ProtocolCap<T> {
    /// Wrap `inner`, capping the protocol version it will admit. `handler`
    /// is the server about to be served over it: the opening discovery is
    /// answered with what that server would have answered itself.
    pub fn new(handler: &impl ServerHandler, inner: T) -> Self {
        let discovery = DiscoverResult::from_server_info(
            handler.supported_protocol_versions().into_owned(),
            handler.get_info(),
        );
        Self {
            inner,
            opening: Opening::new(discovery),
        }
    }
}

impl<T: Transport<RoleServer>> Transport<RoleServer> for ProtocolCap<T> {
    type Error = T::Error;

    fn send(
        &mut self,
        item: TxJsonRpcMessage<RoleServer>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        self.inner.send(item)
    }

    async fn receive(&mut self) -> Option<ClientJsonRpcMessage> {
        loop {
            let mut message = self.inner.receive().await?;
            clamp(&mut message);
            let Some(reply) = self.opening.reply_to(&message) else {
                return Some(message);
            };
            self.inner.send(reply).await.ok()?;
        }
    }

    fn close(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.inner.close()
    }
}

#[cfg(test)]
#[path = "protocol_cap.tests.rs"]
mod tests;
