//! The cockpit's workbench port, from the project's `.mcp.json`.

use std::path::Path;

use magistr_cockpit::workbench::{McpWorkbench, WorkbenchPort};

/// The server entry the cockpit speaks to.
const SERVER: &str = "workbench-mcp";

/// The workbench port for `project_dir`, read off its `.mcp.json`
/// `mcpServers` table — the same file the agent's own servers come from
/// ([`project_mcp_servers`](crate::agent::project_mcp_servers)), so the
/// cockpit and the agent reach the same workbench.
///
/// No `.mcp.json`, no `workbench-mcp` entry, or an entry without a
/// command yields `None` and the session runs without a workbench, as
/// it did before. The port itself does not connect here: it spawns the
/// server on the first call that needs one, so a workspace whose server
/// cannot start costs a line on the stage rather than the cockpit.
#[must_use]
pub fn workbench_port(project_dir: &Path) -> Option<Box<dyn WorkbenchPort>> {
    let raw = std::fs::read_to_string(project_dir.join(".mcp.json")).ok()?;
    let doc: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let spec = doc["mcpServers"].get(SERVER)?;
    let command = spec["command"].as_str()?;
    let args = spec["args"]
        .as_array()
        .map(|args| {
            args.iter()
                .filter_map(|arg| arg.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let env = spec["env"]
        .as_object()
        .map(|env| {
            env.iter()
                .filter_map(|(key, value)| {
                    value.as_str().map(|value| (key.clone(), value.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default();
    // The journal CLI runs with the project as its working directory,
    // so the portable "." root in the launch line resolves against it.
    Some(Box::new(
        McpWorkbench::new(command, args, env).with_workspace(project_dir),
    ))
}

#[cfg(test)]
#[path = "workbench_port.tests.rs"]
mod tests;
