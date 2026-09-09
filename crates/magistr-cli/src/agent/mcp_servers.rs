//! The project's `.mcp.json` as ACP `mcpServers` for `session/new`.
//!
//! Without this the agent would assemble its tool surface from the CLI's
//! own config — the user's global servers — and the project's servers
//! (`magistr mcp-serve`, workbench-mcp, think-and-ship) would only be
//! present if the CLI happened to trust the project file. Passing them on
//! the session request makes them a guarantee: the in-binary claude
//! provider hands the CLI exactly these (`--strict-mcp-config`). Only stdio
//! servers are mapped:
//! HTTP/SSE transports are gated on agent capabilities the client does not
//! negotiate here.

use std::path::Path;

use agent_client_protocol::schema::v1::{EnvVariable, McpServer, McpServerStdio};

/// Read `<project_dir>/.mcp.json` and map its stdio servers. A missing or
/// unparsable file yields no servers — the agent then behaves as before.
#[must_use]
pub fn project_mcp_servers(project_dir: &Path) -> Vec<McpServer> {
    let raw = match std::fs::read_to_string(project_dir.join(".mcp.json")) {
        Ok(raw) => raw,
        Err(_) => return Vec::new(),
    };
    match serde_json::from_str::<serde_json::Value>(&raw) {
        Ok(doc) => mcp_servers_from_value(&doc),
        Err(_) => Vec::new(),
    }
}

/// Map a parsed `.mcp.json` document (`{"mcpServers": {name: {command, args?, env?}}}`).
pub(crate) fn mcp_servers_from_value(doc: &serde_json::Value) -> Vec<McpServer> {
    let Some(servers) = doc["mcpServers"].as_object() else {
        return Vec::new();
    };
    servers
        .iter()
        .filter_map(|(name, spec)| {
            let command = spec["command"].as_str()?;
            let args: Vec<String> = spec["args"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            let env: Vec<EnvVariable> = spec["env"]
                .as_object()
                .map(|e| {
                    e.iter()
                        .filter_map(|(k, v)| v.as_str().map(|v| EnvVariable::new(k, v)))
                        .collect()
                })
                .unwrap_or_default();
            Some(McpServer::Stdio(
                McpServerStdio::new(name, command).args(args).env(env),
            ))
        })
        .collect()
}

#[cfg(test)]
#[path = "mcp_servers.tests.rs"]
mod tests;
