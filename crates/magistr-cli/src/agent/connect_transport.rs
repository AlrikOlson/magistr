//! Spawning the ACP agent for an [`AgentConfig`] and completing the
//! handshake — the launch shared by the legacy backend and the cockpit.

use magistr_acp::{AcpSessionMode, AcpTransport};
use tracing::debug;

use super::AgentConfig;
use crate::error::{MagistrError, Result};

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
    // Resolve the agent runtime (binary + auth) in one pass.
    // Fails fast with actionable guidance if no authenticated runtime is available.
    let runtime = config.resolve_runtime()?;

    // Translate tool names for this runtime's expected format, then merge into CLI args.
    let translated_tools = super::translate_tools_for_runtime(&config.allowed_tools, &runtime.auth);
    let mut cli_args = runtime.args;
    // Deduplicate translated names (e.g. Glob + Grep both → "read" for Copilot)
    let mut seen = std::collections::HashSet::new();
    for tool in &translated_tools {
        if seen.insert(tool.clone()) {
            cli_args.push(format!("--allow-tool={tool}"));
        }
    }

    let cwd = config.cwd.clone();

    // Determine session mode from config
    let session_mode = if let Some(ref session_id) = config.resume {
        AcpSessionMode::Load {
            session_id: session_id.clone(),
        }
    } else {
        AcpSessionMode::New
    };

    // The project's own MCP servers ride on session/new so the agent
    // always has them: the in-binary claude provider passes exactly these
    // to the CLI (--mcp-config + --strict-mcp-config), never these plus
    // every user-scope one.
    let mcp_servers = super::mcp_servers::project_mcp_servers(&cwd);
    debug!(
        count = mcp_servers.len(),
        "project MCP servers for the ACP session"
    );
    let mut transport = AcpTransport::new_with_args(runtime.path, cli_args, Some(cwd))
        .map_err(|e| MagistrError::Acp(format!("Failed to create ACP transport: {e}")))?
        .session_mode(session_mode)
        .mcp_servers(mcp_servers)
        .local_effects();

    transport
        .connect()
        .await
        .map_err(|e| MagistrError::Acp(format!("Failed to connect ACP transport: {e}")))?;
    debug!("ACP transport connected");

    // Register child PGID for signal-handler cleanup (orphan prevention)
    #[cfg(unix)]
    crate::utils::child_guard::register_child_pgid(transport.child_pgid());

    Ok(transport)
}

#[cfg(test)]
#[path = "connect_transport.tests.rs"]
mod tests;
