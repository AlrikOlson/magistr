//! Spawning the ACP agent for an [`AgentConfig`] and completing the
//! handshake — the launch shared by the legacy backend and the cockpit.

use magistr_acp::AcpTransport;

use super::AgentConfig;
use crate::error::Result;

/// Detect the authenticated agent runtime, translate the config's
/// pre-approved tools into its `--allow-tool` arguments, pick new-or-resume
/// from the config, attach the project's `.mcp.json` servers, spawn the
/// agent and complete the handshake. The child's process group is
/// registered with the signal-handler guard so an orphan never survives.
///
/// # Errors
///
/// No authenticated runtime, a transport that cannot be created, or a
/// handshake that fails or times out.
pub async fn connect_transport(config: &AgentConfig) -> Result<AcpTransport> {
    config.clone().resolve()?.connect_transport().await
}

#[cfg(test)]
#[path = "connect_transport.tests.rs"]
mod tests;
