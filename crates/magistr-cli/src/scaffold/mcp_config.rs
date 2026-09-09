use std::collections::HashMap;

use crate::error::{MagistrError, MapMagistrErr, Result};
use crate::mcp_target::McpTarget;

/// Serialize launch values rather than interpolating user paths into JSON.
/// Machine-specific executable discovery belongs to `mcp-launch`, never here.
pub(super) fn build(vars: &HashMap<String, String>) -> Result<serde_json::Value> {
    let root = vars.get("project_root").map_or(".", String::as_str);
    let mut workbench = McpTarget::Workbench.mcp_entry(root);
    // Retain the existing template API for intentional caller overrides. A
    // command-only override takes just the root, not another resolver's argv.
    if vars.contains_key("workbench_command") || vars.contains_key("workbench_args") {
        let command = vars
            .get("workbench_command")
            .map_or("workbench-mcp", String::as_str);
        let args = match vars.get("workbench_args") {
            Some(fragment) => serde_json::from_str::<Vec<String>>(&format!("[{fragment}]"))
                .with_context(MagistrError::Config)?,
            None => vec![root.to_owned()],
        };
        workbench = serde_json::json!({"command": command, "args": args, "type": "stdio"});
    }
    Ok(serde_json::json!({"mcpServers": {
        "magistr": McpTarget::Magistr.mcp_entry(root),
        "workbench-mcp": workbench,
    }}))
}
