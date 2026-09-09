//! Explicit per-session cwd, tools, and approval policy overrides.

use crate::{AgentError, SessionStart};
use agent_client_protocol::schema::v1::McpServer;
use serde_json::{Value, json};

pub(crate) struct ThreadSpec;

impl ThreadSpec {
    pub fn params(start: &SessionStart, model: &str) -> Result<Value, AgentError> {
        let mut servers = serde_json::Map::new();
        for server in &start.mcp_servers {
            let (name, config) = match server {
                McpServer::Stdio(server) => {
                    let env: serde_json::Map<_, _> = server
                        .env
                        .iter()
                        .map(|entry| (entry.name.clone(), json!(entry.value)))
                        .collect();
                    (
                        server.name.clone(),
                        json!({"command":server.command,"args":server.args,"env":env}),
                    )
                }
                McpServer::Http(server) => {
                    let headers: serde_json::Map<_, _> = server
                        .headers
                        .iter()
                        .map(|header| (header.name.clone(), json!(header.value)))
                        .collect();
                    (
                        server.name.clone(),
                        json!({"url":server.url,"http_headers":headers}),
                    )
                }
                McpServer::Sse(server) => {
                    return Err(AgentError::provider(format!(
                        "Codex does not support legacy SSE MCP server {}; use stdio or streamable HTTP",
                        server.name
                    )));
                }
                _ => {
                    return Err(AgentError::provider(
                        "Codex does not support this MCP transport",
                    ));
                }
            };
            if servers.insert(name.clone(), config).is_some() {
                return Err(AgentError::provider(format!(
                    "duplicate Codex MCP server name: {name}"
                )));
            }
        }
        let mut params = json!({"cwd":start.cwd,"modelProvider":"openai","approvalPolicy":"untrusted","approvalsReviewer":"user","sandbox":"workspace-write","config":{"mcp_servers":servers}});
        if start.resume {
            params["threadId"] = json!(start.session_id.to_string());
            params["excludeTurns"] = json!(false);
        } else {
            params["model"] = json!(model);
        }
        Ok(params)
    }
}

#[cfg(test)]
#[path = "thread_spec.tests.rs"]
mod tests;
