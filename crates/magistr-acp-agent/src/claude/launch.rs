//! How one Claude Code process is spawned for one ACP session.

use std::path::PathBuf;

use agent_client_protocol::schema::v1::McpServer;

/// What a session's CLI process is told on its command line: where to run,
/// which session to create or resume, which model to start on, and the MCP
/// servers the client wants — exactly those, thanks to `--strict-mcp-config`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LaunchSpec {
    /// The working directory the client opened the session in.
    pub cwd: PathBuf,
    /// The session's id: `--session-id` for a new one, `--resume` otherwise.
    pub session_id: String,
    /// Whether this resumes a persisted session (`session/load`).
    pub resume: bool,
    /// The model to start on; `None` or `default` leaves the CLI's default.
    pub model: Option<String>,
    /// The MCP servers the client passed on the session request.
    pub mcp_servers: Vec<McpServer>,
    /// Tools the session may run without asking (`--allowedTools`): the
    /// CLI's own rule syntax (`Bash`, `Bash(git *)`, `Edit`, …) or a bare
    /// MCP tool name (`roadmap_status`), which is expanded to
    /// `mcp__<server>__<name>` for every server of the session, since the
    /// CLI only matches an MCP tool by its full name.
    pub allowed_tools: Vec<String>,
}

impl LaunchSpec {
    /// The model id that means "the CLI's own default" — never passed as
    /// `--model`, and what `set_model` resets to.
    pub const DEFAULT_MODEL: &'static str = "default";

    /// The fixed flags every session process gets, before the per-session ones.
    pub const BASE_ARGS: &'static [&'static str] = &[
        "-p",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--permission-prompt-tool",
        "stdio",
        "--permission-mode",
        "default",
        "--strict-mcp-config",
    ];

    /// The full argument list for this session's process.
    pub fn args(&self) -> Vec<String> {
        let mut args: Vec<String> = Self::BASE_ARGS.iter().map(|s| (*s).to_owned()).collect();
        if !self.mcp_servers.is_empty() {
            args.push("--mcp-config".to_owned());
            args.push(mcp_config_json(&self.mcp_servers).to_string());
        }
        let allowed = self.allowed_tool_rules();
        if !allowed.is_empty() {
            args.push("--allowedTools".to_owned());
            args.push(allowed.join(","));
        }
        if let Some(model) = self.model.as_deref().filter(|m| *m != Self::DEFAULT_MODEL) {
            args.push("--model".to_owned());
            args.push(model.to_owned());
        }
        args.push(
            if self.resume {
                "--resume"
            } else {
                "--session-id"
            }
            .to_owned(),
        );
        args.push(self.session_id.clone());
        args
    }
}

impl LaunchSpec {
    /// The allowed-tool rules as the CLI matches them: every name as given,
    /// plus `mcp__<server>__<name>` for each bare lowercase name — the
    /// spelling magistr pre-approves its MCP tools by — on each session
    /// server. A rule for a tool a server does not have never matches.
    fn allowed_tool_rules(&self) -> Vec<String> {
        let servers: Vec<&str> = self
            .mcp_servers
            .iter()
            .filter_map(|s| match s {
                McpServer::Stdio(s) => Some(s.name.as_str()),
                McpServer::Http(s) => Some(s.name.as_str()),
                McpServer::Sse(s) => Some(s.name.as_str()),
                _ => None,
            })
            .collect();
        let mut rules = Vec::new();
        for tool in &self.allowed_tools {
            rules.push(tool.clone());
            let bare_mcp_name = !tool.contains('(')
                && !tool.starts_with("mcp__")
                && tool.chars().next().is_some_and(char::is_lowercase);
            if bare_mcp_name {
                rules.extend(servers.iter().map(|s| format!("mcp__{s}__{tool}")));
            }
        }
        rules
    }
}

/// The `--mcp-config` document: `{"mcpServers": {name: spec}}`, in the shape
/// Claude Code reads from `.mcp.json`.
fn mcp_config_json(servers: &[McpServer]) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for server in servers {
        match server {
            McpServer::Stdio(stdio) => {
                let env: serde_json::Map<String, serde_json::Value> = stdio
                    .env
                    .iter()
                    .map(|e| (e.name.clone(), serde_json::Value::String(e.value.clone())))
                    .collect();
                map.insert(
                    stdio.name.clone(),
                    serde_json::json!({
                        "command": stdio.command.to_string_lossy(),
                        "args": stdio.args,
                        "env": env,
                    }),
                );
            }
            McpServer::Http(http) => {
                map.insert(
                    http.name.clone(),
                    serde_json::json!({"type": "http", "url": http.url, "headers": headers(&http.headers)}),
                );
            }
            McpServer::Sse(sse) => {
                map.insert(
                    sse.name.clone(),
                    serde_json::json!({"type": "sse", "url": sse.url, "headers": headers(&sse.headers)}),
                );
            }
            _ => {}
        }
    }
    serde_json::json!({ "mcpServers": map })
}

fn headers(
    headers: &[agent_client_protocol::schema::v1::HttpHeader],
) -> serde_json::Map<String, serde_json::Value> {
    headers
        .iter()
        .map(|h| (h.name.clone(), serde_json::Value::String(h.value.clone())))
        .collect()
}

#[cfg(test)]
#[path = "launch.tests.rs"]
mod tests;
