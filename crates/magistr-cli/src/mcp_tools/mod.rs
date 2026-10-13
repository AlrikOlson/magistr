//! MCP tool server for magistr-managed file operations.
//!
//! Exposes file management tools (delete, list, write) as an MCP server that
//! Claude Code can call during iterations. Solves the "unable to delete" friction
//! where Claude Code has no native file deletion tool.
//!
//! ## Tools
//!
//! - `magistr_delete` — Delete a file or directory within the project
//! - `magistr_list` — List directory contents
//! - `magistr_gate` — Run quality gate commands (check, test, lint, all)
//! - `magistr_format` — Run the project formatter
//! - `magistr_hooks_list` — List configured lifecycle hooks
//! - `magistr_hooks_add` — Add a lifecycle hook
//! - `magistr_hooks_remove` — Remove a lifecycle hook
//!
//! Roadmap, reasoning and execution tools are the think-and-ship families
//! (`roadmap_*`, `think_*`, `ship_*`, `signal_*`), mounted beside these by
//! [`MagistrService`] — there is no magistr-specific roadmap tool set.
//!
//! ## Security
//!
//! All paths are resolved relative to the project root. Path traversal
//! outside the project is rejected.

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
use magistr_mcp::{SdkMcpServer, SdkMcpTool, ToolResult};
use serde_json::json;
use tracing::debug;

/// Build the magistr MCP server with file management tools.
///
/// `project_root` is the absolute path to the project directory. All tool
/// operations are sandboxed to this directory.
pub fn build_server(project_root: PathBuf) -> SdkMcpServer {
    let root_delete = project_root.clone();
    let root_gate = project_root.clone();
    let root_format = project_root.clone();
    let root_hooks = project_root.clone();
    let root_init = project_root.clone();
    let root_list = project_root;

    let delete_tool = SdkMcpTool::new(
        "magistr_delete",
        "Delete a file or directory within the project. Use this instead of `rm` in bash. \
         Path is relative to the project root.",
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Relative path to delete (e.g. '.claude/rules/old-rule.md' or '.magistr/current-plan.md')"
                },
                "recursive": {
                    "type": "boolean",
                    "description": "If true, delete directories recursively. Default: false.",
                    "default": false
                }
            },
            "required": ["path"]
        }),
        move |input| {
            let root = root_delete.clone();
            Box::pin(async move {
                let rel_path = input["path"].as_str().ok_or_else(|| {
                    magistr_mcp::ToolError::Mcp("missing 'path' parameter".into())
                })?;
                let recursive = input["recursive"].as_bool().unwrap_or(false);
                debug!(
                    tool = "magistr_delete",
                    path = rel_path,
                    recursive,
                    "MCP tool invoked"
                );

                let target = resolve_safe_path(&root, rel_path)?;

                if !target.exists() {
                    return Ok(ToolResult::error(format!(
                        "Path does not exist: {rel_path}"
                    )));
                }

                if target.is_dir() {
                    if recursive {
                        if is_protected_path(&root, &target, rel_path) {
                            return Ok(ToolResult::error(format!(
                                "Refusing to recursively delete protected path '{rel_path}'. \
                                 Remove files individually or use a more specific sub-path."
                            )));
                        }
                        std::fs::remove_dir_all(&target).map_err(|e| {
                            magistr_mcp::ToolError::Mcp(format!(
                                "Failed to delete directory '{rel_path}': {e}"
                            ))
                        })?;
                        Ok(ToolResult::text(format!("Deleted directory: {rel_path}")))
                    } else {
                        // Try non-recursive first (only works on empty dirs)
                        match std::fs::remove_dir(&target) {
                            Ok(()) => Ok(ToolResult::text(format!(
                                "Deleted empty directory: {rel_path}"
                            ))),
                            Err(_) => Ok(ToolResult::error(format!(
                                "Directory '{rel_path}' is not empty. Use recursive: true to delete it."
                            ))),
                        }
                    }
                } else {
                    std::fs::remove_file(&target).map_err(|e| {
                        magistr_mcp::ToolError::Mcp(format!(
                            "Failed to delete file '{rel_path}': {e}"
                        ))
                    })?;
                    Ok(ToolResult::text(format!("Deleted file: {rel_path}")))
                }
            })
        },
    );

    let list_tool = SdkMcpTool::new(
        "magistr_list",
        "List files and directories at a path within the project. \
         Path is relative to the project root.",
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Relative directory path to list (e.g. '.claude/rules'). Defaults to project root.",
                    "default": "."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of entries to return. Default: 500.",
                    "default": 500
                }
            }
        }),
        move |input| {
            let root = root_list.clone();
            Box::pin(async move {
                let rel_path = input["path"].as_str().unwrap_or(".");
                let limit = input["limit"].as_u64().unwrap_or(500) as usize;
                debug!(
                    tool = "magistr_list",
                    path = rel_path,
                    limit,
                    "MCP tool invoked"
                );

                let target = resolve_safe_path(&root, rel_path)?;

                if !target.exists() {
                    return Ok(ToolResult::error(format!(
                        "Path does not exist: {rel_path}"
                    )));
                }

                if !target.is_dir() {
                    return Ok(ToolResult::error(format!("Not a directory: {rel_path}")));
                }

                let mut entries: Vec<String> = Vec::new();
                let read_dir = std::fs::read_dir(&target).map_err(|e| {
                    magistr_mcp::ToolError::Mcp(format!(
                        "Failed to read directory '{rel_path}': {e}"
                    ))
                })?;

                for entry in read_dir {
                    let entry = entry.map_err(|e| {
                        magistr_mcp::ToolError::Mcp(format!("Failed to read entry: {e}"))
                    })?;
                    let name = entry.file_name().to_string_lossy().to_string();
                    let suffix = if entry.path().is_dir() { "/" } else { "" };
                    entries.push(format!("{name}{suffix}"));
                }

                entries.sort();

                if entries.is_empty() {
                    Ok(ToolResult::text(format!("{rel_path}/ (empty)")))
                } else if entries.len() > limit {
                    let total = entries.len();
                    let remaining = total - limit;
                    entries.truncate(limit);
                    entries.push(format!("... and {remaining} more entries"));
                    Ok(ToolResult::text(entries.join("\n")))
                } else {
                    Ok(ToolResult::text(entries.join("\n")))
                }
            })
        },
    );

    // ── Workflow phase tool ──────────────────────────────────────────────────
    let set_phase_tool = set_phase::build_set_phase_tool();

    // ── Quality gate & format tools ─────────────────────────────────────────
    let gate_tool = gate::build_gate_tool(root_gate);
    let format_tool = format::build_format_tool(root_format);

    // ── Init tool ────────────────────────────────────────────────────────────
    let init_tool = init::build_init_tool(root_init);

    let mut server = SdkMcpServer::new("magistr")
        .version(env!("CARGO_PKG_VERSION"))
        .tool(delete_tool)
        .tool(list_tool)
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

/// Resolve a relative path safely within the project root.
///
/// Rejects paths that would escape the project directory via `..` traversal.
fn resolve_safe_path(root: &Path, rel_path: &str) -> Result<PathBuf, magistr_mcp::ToolError> {
    use std::path::Component;

    // Reject absolute paths
    if Path::new(rel_path).is_absolute() {
        return Err(magistr_mcp::ToolError::Mcp(
            "Absolute paths are not allowed. Use a path relative to the project root.".into(),
        ));
    }

    let canonical_root = if root.exists() {
        root.canonicalize().map_err(|e| {
            magistr_mcp::ToolError::Mcp(format!("Failed to resolve project root: {e}"))
        })?
    } else {
        normalize_path(root)
    };

    let mut resolved = canonical_root.clone();
    for component in Path::new(rel_path).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop();
                if !resolved.starts_with(&canonical_root) {
                    return Err(magistr_mcp::ToolError::Mcp(format!(
                        "Path '{rel_path}' escapes the project directory. Access denied."
                    )));
                }
            }
            Component::Normal(part) => {
                resolved.push(part);
                if resolved.exists() {
                    resolved = resolved.canonicalize().map_err(|e| {
                        magistr_mcp::ToolError::Mcp(format!(
                            "Failed to resolve path '{rel_path}': {e}"
                        ))
                    })?;
                    if !resolved.starts_with(&canonical_root) {
                        return Err(magistr_mcp::ToolError::Mcp(format!(
                            "Path '{rel_path}' escapes the project directory. Access denied."
                        )));
                    }
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(magistr_mcp::ToolError::Mcp(
                    "Absolute paths are not allowed. Use a path relative to the project root."
                        .into(),
                ));
            }
        }
    }

    if !resolved.starts_with(&canonical_root) {
        return Err(magistr_mcp::ToolError::Mcp(format!(
            "Path '{rel_path}' escapes the project directory. Access denied."
        )));
    }

    Ok(resolved)
}

/// Top-level directory names that must not be recursively deleted.
///
/// These are critical project directories whose accidental removal would
/// cause data loss or break the project irreparably.
const PROTECTED_DIRS: &[&str] = &[
    ".git",
    "src",
    "lib",
    "app",
    "pkg",
    "crates",
    "packages",
    "node_modules",
];

/// Check whether a recursive delete target is a protected path.
///
/// Protected paths are:
/// 1. The project root itself (resolved path == root)
/// 2. Top-level directories in [`PROTECTED_DIRS`]
fn is_protected_path(root: &Path, target: &Path, rel_path: &str) -> bool {
    // Normalize the relative path to get the first component
    let normalized = normalize_path(Path::new(rel_path));
    let first_component = normalized.components().next().and_then(|c| match c {
        std::path::Component::Normal(s) => s.to_str(),
        _ => None,
    });

    // Reject if target resolves to the project root
    if let (Ok(canon_root), Ok(canon_target)) = (root.canonicalize(), target.canonicalize())
        && canon_root == canon_target
    {
        return true;
    }
    // Fallback for non-existent paths: check normalized form
    let norm_root = normalize_path(root);
    let norm_target = normalize_path(target);
    if norm_root == norm_target {
        return true;
    }

    // Check if it's a single-component path matching a protected name
    let component_count = normalized.components().count();
    if component_count == 1
        && let Some(name) = first_component
    {
        return PROTECTED_DIRS.contains(&name);
    }

    false
}

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

/// Normalize a path by resolving `.` and `..` components without requiring
/// the path to exist on disk.
fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            other => components.push(other),
        }
    }
    components.iter().collect()
}

/// Run the MCP server on stdio, reading JSON-RPC requests from stdin
/// and writing responses to stdout.
///
/// This is the entry point for `magistr mcp-serve`.
pub async fn run_stdio_server(project_root: PathBuf) -> eyre::Result<()> {
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
    .run()
    .await
}

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
