//! MCP server configuration writer.
//!
//! Generates a temporary `.mcp.json` that includes magistr's tool server,
//! making magistr tools available to agent sessions without requiring
//! the project to have pre-existing MCP configuration.

use serde_json::json;
use std::path::{Path, PathBuf};

use crate::mcp_target::McpTarget;

/// Write a temporary `.mcp.json` that includes the magistr MCP server.
///
/// If the project already has `.mcp.json`, merges magistr's entry into it.
/// Returns the path to the (possibly temporary) config file and whether
/// it was created (needs cleanup) or merged into an existing file.
///
/// # Arguments
/// * `project_dir` — The project root directory
/// * `magistr_binary` — Path to the magistr binary (for `magistr mcp-serve`)
pub fn write_mcp_config(
    project_dir: &Path,
    magistr_binary: &Path,
) -> std::io::Result<McpConfigGuard> {
    let mcp_path = project_dir.join(".mcp.json");
    let magistr_entry = build_magistr_entry(magistr_binary, project_dir);

    if mcp_path.exists() {
        // Merge into existing config
        let existing = std::fs::read_to_string(&mcp_path)?;
        let mut config: serde_json::Value =
            serde_json::from_str(&existing).unwrap_or_else(|_| json!({}));

        let servers = config
            .as_object_mut()
            .and_then(|o| o.get_mut("mcpServers"))
            .and_then(|s| s.as_object_mut());

        if let Some(servers) = servers
            && !servers.contains_key("magistr")
        {
            servers.insert("magistr".to_string(), magistr_entry);
            std::fs::write(&mcp_path, serde_json::to_string_pretty(&config)?)?;
            return Ok(McpConfigGuard {
                path: mcp_path,
                action: McpConfigAction::Merged,
            });
        }
        // Already has magistr entry or unexpected structure — use as-is
        Ok(McpConfigGuard {
            path: mcp_path,
            action: McpConfigAction::Unchanged,
        })
    } else {
        // Create new temporary config
        let config = json!({
            "mcpServers": {
                "magistr": magistr_entry
            }
        });
        std::fs::write(&mcp_path, serde_json::to_string_pretty(&config)?)?;
        Ok(McpConfigGuard {
            path: mcp_path,
            action: McpConfigAction::Created,
        })
    }
}

/// Build the magistr MCP server entry for `.mcp.json`.
fn build_magistr_entry(magistr_binary: &Path, project_dir: &Path) -> serde_json::Value {
    json!({
        "type": "stdio",
        "command": magistr_binary.to_string_lossy(),
        "args": ["mcp-serve", project_dir.to_string_lossy()]
    })
}

/// Build the workbench-mcp server entry for `.mcp.json`: the native workspace
/// bridge of this same binary, so it is always launchable.
fn build_workbench_entry(project_dir: &Path) -> serde_json::Value {
    let root = project_dir
        .canonicalize()
        .unwrap_or_else(|_| project_dir.to_path_buf())
        .to_string_lossy()
        .to_string();
    McpTarget::Workbench.mcp_entry(&root)
}

/// Write an MCP config with magistr + workbench-mcp.
///
/// Used by the interview to give the AI agent access to both
/// `magistr_recommend` and workbench codebase exploration tools.
pub fn write_interview_mcp_config(
    project_dir: &Path,
    magistr_binary: &Path,
) -> std::io::Result<McpConfigGuard> {
    let mcp_path = project_dir.join(".mcp.json");

    if mcp_path.exists() {
        // Merge into existing config
        let existing = std::fs::read_to_string(&mcp_path)?;
        let mut config: serde_json::Value =
            serde_json::from_str(&existing).unwrap_or_else(|_| json!({}));

        let servers = config
            .as_object_mut()
            .and_then(|o| o.get_mut("mcpServers"))
            .and_then(|s| s.as_object_mut());

        if let Some(servers) = servers {
            let mut modified = false;
            if !servers.contains_key("magistr") {
                servers.insert(
                    "magistr".to_string(),
                    build_magistr_entry(magistr_binary, project_dir),
                );
                modified = true;
            }
            if !servers.contains_key("workbench-mcp") {
                servers.insert(
                    "workbench-mcp".to_string(),
                    build_workbench_entry(project_dir),
                );
                modified = true;
            }
            if modified {
                std::fs::write(&mcp_path, serde_json::to_string_pretty(&config)?)?;
                return Ok(McpConfigGuard {
                    path: mcp_path,
                    action: McpConfigAction::Merged,
                });
            }
        }
        Ok(McpConfigGuard {
            path: mcp_path,
            action: McpConfigAction::Unchanged,
        })
    } else {
        let mut servers = serde_json::Map::new();
        servers.insert(
            "magistr".to_string(),
            build_magistr_entry(magistr_binary, project_dir),
        );
        servers.insert(
            "workbench-mcp".to_string(),
            build_workbench_entry(project_dir),
        );
        let config = json!({ "mcpServers": servers });
        std::fs::write(&mcp_path, serde_json::to_string_pretty(&config)?)?;
        Ok(McpConfigGuard {
            path: mcp_path,
            action: McpConfigAction::Created,
        })
    }
}

/// Guard for temporary MCP config — tracks what was done for cleanup.
#[derive(Debug)]
pub struct McpConfigGuard {
    /// Path to the `.mcp.json` file.
    pub path: PathBuf,
    /// What action was taken.
    pub action: McpConfigAction,
}

impl McpConfigGuard {
    /// Clean up: remove file if we created it, remove magistr entry if we merged.
    pub fn cleanup(&self) {
        match self.action {
            McpConfigAction::Created => {
                let _ = std::fs::remove_file(&self.path);
            }
            McpConfigAction::Merged => {
                // Remove the magistr entry we added
                if let Ok(content) = std::fs::read_to_string(&self.path)
                    && let Ok(mut config) = serde_json::from_str::<serde_json::Value>(&content)
                    && let Some(servers) = config
                        .as_object_mut()
                        .and_then(|o| o.get_mut("mcpServers"))
                        .and_then(|s| s.as_object_mut())
                {
                    servers.remove("magistr");
                    let _ = std::fs::write(
                        &self.path,
                        serde_json::to_string_pretty(&config).unwrap_or_default(),
                    );
                }
            }
            McpConfigAction::Unchanged => {}
        }
    }
}

impl Drop for McpConfigGuard {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// What action was taken when setting up MCP config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpConfigAction {
    /// Created a new `.mcp.json` file (needs removal on cleanup).
    Created,
    /// Merged magistr entry into existing `.mcp.json` (needs removal of entry on cleanup).
    Merged,
    /// Existing config already had magistr entry — nothing changed.
    Unchanged,
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "server.tests.rs"]
mod tests;
