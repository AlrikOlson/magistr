//! The agent on this process's stdio.

use agent_client_protocol::Stdio;

use crate::{Provider, serve};

/// Serve `provider` as an ACP agent over this process's stdin and stdout —
/// what `magistr acp-agent --provider <name>` runs — until the client closes
/// the connection. Nothing else may write to stdout while this runs: the
/// transport is newline-delimited JSON-RPC, and a stray line corrupts it.
pub async fn serve_stdio(provider: impl Provider) -> Result<(), agent_client_protocol::Error> {
    serve(provider, Stdio::new()).await
}
