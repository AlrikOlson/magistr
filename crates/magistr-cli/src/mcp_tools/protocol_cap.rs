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

use rmcp::model::{ClientJsonRpcMessage, ClientRequest, GetMeta, ProtocolVersion};
use rmcp::service::{RoleServer, TxJsonRpcMessage};
use rmcp::transport::Transport;

/// The era served in place of 2026-07-28+: the newest version whose result
/// shapes carry no `resultType`, and the one the proven-good standalone
/// Claude Code pairing negotiates.
const SERVED: ProtocolVersion = ProtocolVersion::V_2025_06_18;

/// Clamp `message`'s requested MCP protocol version below 2026-07-28: the
/// `initialize` params, and the per-request `_meta` version that can re-raise
/// the era on any later request.
fn clamp(message: &mut ClientJsonRpcMessage) {
    let boundary = ProtocolVersion::V_2026_07_28;
    let ClientJsonRpcMessage::Request(request) = message else {
        return;
    };
    if let ClientRequest::InitializeRequest(init) = &mut request.request
        && init.params.protocol_version.as_str() >= boundary.as_str()
    {
        init.params.protocol_version = SERVED;
    }
    let meta = request.request.get_meta_mut();
    if meta
        .protocol_version()
        .is_some_and(|v| v.as_str() >= boundary.as_str())
    {
        meta.set_protocol_version(SERVED);
    }
}

/// A [`Transport`] wrapper that applies [`clamp`] to every inbound message,
/// pinning the session to a protocol era every Claude client accepts.
pub struct ProtocolCap<T> {
    inner: T,
}

impl<T> ProtocolCap<T> {
    /// Wrap `inner`, capping the protocol version it will admit.
    pub const fn new(inner: T) -> Self {
        Self { inner }
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
        let mut message = self.inner.receive().await?;
        clamp(&mut message);
        Some(message)
    }

    fn close(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.inner.close()
    }
}

#[cfg(test)]
#[path = "protocol_cap.tests.rs"]
mod tests;
