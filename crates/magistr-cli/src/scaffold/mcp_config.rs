use crate::error::Result;
use crate::mcp_target::McpTarget;
use std::collections::HashMap;

/// Serialize the one portable launch command, preserving paths as literal argv.
pub(super) fn build(vars: &HashMap<String, String>) -> Result<serde_json::Value> {
    let root = vars.get("project_root").map_or(".", String::as_str);
    Ok(serde_json::json!({"mcpServers":{"magistr":McpTarget.mcp_entry(root)}}))
}
