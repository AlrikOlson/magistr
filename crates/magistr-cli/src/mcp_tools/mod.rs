//! Native workflow tools. All source operations belong to the workspace owner.

mod format;
mod format_runs;
pub(crate) mod gate;
pub(crate) mod hooks;
mod init;
mod owner_attach;
mod owner_link;
pub(crate) mod owner_policy;
#[cfg(unix)]
pub(crate) mod owner_stdio;
pub mod protocol_cap;
mod roadmap_export;
mod service;
mod set_phase;
mod view_context;

pub use service::MagistrService;

use std::path::{Path, PathBuf};

use color_eyre::eyre;
use magistr_mcp::SdkMcpServer;

/// Build the magistr MCP server with file management tools.
///
/// `project_root` is the absolute path to the project directory. All tool
/// operations are sandboxed to this directory.
pub fn build_server(project_root: PathBuf) -> SdkMcpServer {
    let root_gate = project_root.clone();
    let root_format = project_root.clone();
    let root_hooks = project_root.clone();
    let root_init = project_root;
    // ── Workflow phase tool ──────────────────────────────────────────────────
    let set_phase_tool = set_phase::build_set_phase_tool();

    // ── Quality gate & format tools ─────────────────────────────────────────
    let gate_tool = gate::build_gate_tool(root_gate);
    let format_tool = format::build_format_tool(root_format);

    // ── Init tool ────────────────────────────────────────────────────────────
    let init_tool = init::build_init_tool(root_init);

    let mut server = SdkMcpServer::new("magistr")
        .version(env!("CARGO_PKG_VERSION"))
        .tool(set_phase_tool)
        .tool(gate_tool)
        .tool(format_tool)
        .tool(init_tool);

    // ── Lifecycle hooks management tools ─────────────────────────────────
    let hooks_tools = hooks::build_hooks_tools(root_hooks);
    for tool in hooks_tools {
        server = server.tool(tool);
    }

    // Register all tools from the tool registry (e.g. magistr_recommend)
    let registry = crate::tools::ToolRegistry::new();
    for tool in registry.build_mcp_tools() {
        server = server.tool(tool);
    }

    server
}

// ── Shared helpers ──────────────────────────────────────────────────────────

/// Validate that required executables are available on PATH.
///
/// Returns a list of executable names that were NOT found.
fn validate_required_executables(executables: &[&str]) -> Vec<String> {
    executables
        .iter()
        .filter(|&&exe| which::which(exe).is_err())
        .map(|&exe| exe.to_string())
        .collect()
}

/// Extract the executable name (first word) from a shell command string.
fn extract_executable(command: &str) -> Option<&str> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Skip env vars like KEY=val at the start
    trimmed.split_whitespace().find(|part| !part.contains('='))
}

/// Validate that executables required by the project config are on PATH.
///
/// Reads `.magistr/project.json`, extracts the first word from each gate/format
/// command, and checks availability with `which`. Returns a list of warning
/// strings for any missing executables.
fn validate_project_executables(project_root: &Path) -> Vec<String> {
    let config_path = match crate::paths::project_json(project_root) {
        Ok(path) => path,
        Err(error) => return vec![error.to_string()],
    };
    let config_str = match std::fs::read_to_string(&config_path) {
        Ok(s) => s,
        Err(_) => return vec![], // No config file — nothing to validate
    };
    let config: serde_json::Value = match serde_json::from_str(&config_str) {
        Ok(v) => v,
        Err(_) => return vec![], // Malformed config — skip validation
    };

    let mut executables_to_check: Vec<&str> = Vec::new();

    // Extract executables from quality gate commands
    if let Some(qg) = config.get("quality_gate") {
        for key in &["check", "test", "lint"] {
            if let Some(cmd) = qg.get(key).and_then(|v| v.as_str())
                && let Some(exe) = extract_executable(cmd)
            {
                executables_to_check.push(exe);
            }
        }
    }

    // Extract executable from format command
    if let Some(fmt) = config.get("format")
        && let Some(cmd) = fmt.get("command").and_then(|v| v.as_str())
        && let Some(exe) = extract_executable(cmd)
    {
        executables_to_check.push(exe);
    }

    // Deduplicate
    executables_to_check.sort_unstable();
    executables_to_check.dedup();

    let missing = validate_required_executables(&executables_to_check);
    missing
        .into_iter()
        .map(|exe| format!("Required executable '{exe}' not found on PATH"))
        .collect()
}

/// Run the MCP server on stdio, reading JSON-RPC requests from stdin
/// and writing responses to stdout.
///
/// Internal workflow connection used by the unified MCP service.
pub(crate) async fn run_transport(
    project_root: PathBuf,
    input: impl tokio::io::AsyncRead + Unpin,
    output: impl tokio::io::AsyncWrite + Unpin,
) -> eyre::Result<()> {
    // Validate required executables at startup; stderr is the log channel of
    // a stdio MCP server, stdout carries only the transport.
    for warning in validate_project_executables(&project_root) {
        tracing::warn!(target: "magistr::mcp", "{warning}");
    }

    // The think-and-ship engines resolve identity and run gates relative to
    // the working directory; pin it to the project root the client named.
    if let Err(e) = std::env::set_current_dir(&project_root) {
        tracing::warn!(target: "magistr::mcp", "could not enter {}: {e}", project_root.display());
    }

    // Capture one writer configuration for admission and every later reconnect.
    let config = magistr_core::evidence::writer_config_from_env(&project_root);
    #[cfg(unix)]
    let config = config.with_project_socket_default();
    let options = owner_link::Options {
        root: project_root,
        config,
    };
    use crate::mcp_bridge::{Bridge, BridgeLink};
    let link = owner_link::Link::open(&options)
        .await
        .map_err(|e| eyre::eyre!("{e}"))?;
    Bridge::new(
        options,
        "workflow".to_owned(),
        owner_policy::Policy,
        Ok(link),
    )
    .run_transport(input, output)
    .await
}

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
