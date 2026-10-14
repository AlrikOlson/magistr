//! `magistr_hooks_*` MCP tools — lifecycle hook management.
//!
//! Three tools for managing lifecycle hooks at runtime:
//! - `magistr_hooks_list` — list configured hooks grouped by event
//! - `magistr_hooks_add` — add a new hook (validates, deduplicates, persists)
//! - `magistr_hooks_remove` — remove a hook by index or (event, tool) pair

use std::path::PathBuf;

use magistr_engine::{HookAction, LifecycleEvent, LifecycleHook, OnFailure};
use magistr_mcp::{SdkMcpTool, ToolResult};
use serde_json::json;
use tracing::debug;

use crate::types::ProjectConfig;

/// Build the three hooks management MCP tools.
pub(super) fn build_hooks_tools(project_root: PathBuf) -> Vec<SdkMcpTool> {
    let root_list = project_root.clone();
    let root_add = project_root.clone();
    let root_remove = project_root;

    vec![
        build_hooks_list_tool(root_list),
        build_hooks_add_tool(root_add),
        build_hooks_remove_tool(root_remove),
    ]
}

// ── magistr_hooks_list ──────────────────────────────────────────────────

fn build_hooks_list_tool(project_root: PathBuf) -> SdkMcpTool {
    SdkMcpTool::new(
        "magistr_hooks_list",
        "List all configured lifecycle hooks, grouped by event. \
         Shows the tool name, arguments, on_failure strategy, and description for each hook.",
        json!({
            "type": "object",
            "properties": {}
        }),
        move |_input| {
            let root = project_root.clone();
            Box::pin(async move {
                debug!(tool = "magistr_hooks_list", "MCP tool invoked");

                let config = match load_project_config(&root) {
                    Ok(c) => c,
                    Err(e) => return Ok(ToolResult::error(e)),
                };

                if config.lifecycle_hooks.is_empty() {
                    return Ok(ToolResult::text(
                        "No lifecycle hooks configured.\n\n\
                         Use magistr_hooks_add to add hooks that fire on workflow events \
                         (e.g., on_enter(verify), on_gate_pass).",
                    ));
                }

                Ok(ToolResult::text(format_hooks_list(&config.lifecycle_hooks)))
            })
        },
    )
}

// ── magistr_hooks_add ───────────────────────────────────────────────────

fn build_hooks_add_tool(project_root: PathBuf) -> SdkMcpTool {
    SdkMcpTool::new(
        "magistr_hooks_add",
        "Add a lifecycle hook (MCP tool or shell command). Validates the event, \
         rejects duplicates and dangerous commands, persists to project.json.",
        json!({
            "type": "object",
            "properties": {
                "event": {
                    "type": "string",
                    "description": "Lifecycle event to trigger on. One of: \
                        on_enter(research), on_enter(plan), on_enter(execute), on_enter(verify), on_enter(done), \
                        on_exit(research), on_exit(plan), on_exit(execute), on_exit(verify), on_exit(done), \
                        on_iteration_start, on_iteration_end, on_gate_pass, on_gate_fail"
                },
                "tool": {
                    "type": "string",
                    "description": "MCP tool name to invoke (e.g., 'magistr_format', 'scope_search'). \
                        Mutually exclusive with 'command'."
                },
                "command": {
                    "type": "string",
                    "description": "Shell command to run directly (e.g., 'cargo fmt', 'npm run lint'). \
                        Mutually exclusive with 'tool'."
                },
                "arguments": {
                    "type": "object",
                    "description": "Optional arguments for MCP tool hooks. Ignored for shell commands."
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Timeout in seconds for shell command hooks. Default: 30."
                },
                "description": {
                    "type": "string",
                    "description": "Optional description shown in the agent system prompt."
                },
                "on_failure": {
                    "type": "string",
                    "description": "What to do when the hook fails. Default: continue.",
                    "enum": ["continue", "fail_fast", "warn"],
                    "default": "continue"
                }
            },
            "required": ["event"]
        }),
        move |input| {
            let root = project_root.clone();
            Box::pin(async move {
                let event_str = input["event"].as_str().unwrap_or("");
                let tool_name = input["tool"].as_str().unwrap_or("");
                let command = input["command"].as_str().unwrap_or("");

                debug!(
                    tool = "magistr_hooks_add",
                    event = event_str,
                    hook_tool = tool_name,
                    hook_command = command,
                    "MCP tool invoked"
                );

                if event_str.is_empty() {
                    return Ok(ToolResult::error("Missing required field: event"));
                }

                // Must have exactly one of tool or command
                if tool_name.is_empty() && command.is_empty() {
                    return Ok(ToolResult::error(
                        "Provide either 'tool' (MCP tool name) or 'command' (shell command).",
                    ));
                }
                if !tool_name.is_empty() && !command.is_empty() {
                    return Ok(ToolResult::error(
                        "'tool' and 'command' are mutually exclusive. Use one or the other.",
                    ));
                }

                let event = match parse_event(event_str) {
                    Some(e) => e,
                    None => {
                        return Ok(ToolResult::error(format!(
                            "Invalid event '{event_str}'. Valid events: \
                             on_enter(<phase>), on_exit(<phase>), \
                             on_iteration_start, on_iteration_end, \
                             on_gate_pass, on_gate_fail. \
                             Valid phases: research, plan, execute, verify, done."
                        )));
                    }
                };

                let action = if !tool_name.is_empty() {
                    let arguments = input
                        .get("arguments")
                        .filter(|v| !v.is_null())
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);
                    HookAction::McpTool {
                        tool: tool_name.to_string(),
                        arguments,
                    }
                } else {
                    let timeout_secs = input["timeout_secs"].as_u64();
                    HookAction::ShellCommand {
                        command: command.to_string(),
                        timeout_secs,
                    }
                };

                let description = input["description"].as_str().map(String::from);

                let on_failure = match input["on_failure"].as_str() {
                    Some("fail_fast") => OnFailure::FailFast,
                    Some("warn") => OnFailure::Warn,
                    _ => OnFailure::Continue,
                };

                let hook = LifecycleHook {
                    event,
                    action,
                    description,
                    on_failure,
                };

                // Validate (security checks for shell commands + dedup)
                if let Err(e) = magistr_engine::LifecycleHooksConfig::new(vec![hook.clone()]) {
                    return Ok(ToolResult::error(format!("Hook validation failed: {e}")));
                }

                let mut config = match load_project_config(&root) {
                    Ok(c) => c,
                    Err(e) => return Ok(ToolResult::error(e)),
                };

                // Check for duplicates
                if config
                    .lifecycle_hooks
                    .iter()
                    .any(|h| h.event == hook.event && h.display_name() == hook.display_name())
                {
                    return Ok(ToolResult::error(format!(
                        "Duplicate hook: event={}, action={}. A hook with this event and action already exists.",
                        hook.event,
                        hook.display_name()
                    )));
                }

                let event_display = hook.event.to_string();
                let action_display = hook.display_name().to_owned();
                config.lifecycle_hooks.push(hook);

                if let Err(e) = save_project_config(&root, &config) {
                    return Ok(ToolResult::error(e));
                }

                Ok(ToolResult::text(format!(
                    "✓ Hook added: {action_display} on {event_display}\n\n\
                     Total hooks: {}",
                    config.lifecycle_hooks.len()
                )))
            })
        },
    )
}

// ── magistr_hooks_remove ────────────────────────────────────────────────

fn build_hooks_remove_tool(project_root: PathBuf) -> SdkMcpTool {
    SdkMcpTool::new(
        "magistr_hooks_remove",
        "Remove a lifecycle hook by index (0-based) or by event + name pair.",
        json!({
            "type": "object",
            "properties": {
                "index": {
                    "type": "integer",
                    "description": "0-based index of the hook to remove (from magistr_hooks_list output)"
                },
                "event": {
                    "type": "string",
                    "description": "Event of the hook to remove (used with 'name' for lookup)"
                },
                "name": {
                    "type": "string",
                    "description": "Action name (tool name or shell command) of the hook to remove"
                }
            }
        }),
        move |input| {
            let root = project_root.clone();
            Box::pin(async move {
                debug!(tool = "magistr_hooks_remove", "MCP tool invoked");

                let mut config = match load_project_config(&root) {
                    Ok(c) => c,
                    Err(e) => return Ok(ToolResult::error(e)),
                };

                let removed = if let Some(idx) = input["index"].as_u64() {
                    let idx = idx as usize;
                    if idx >= config.lifecycle_hooks.len() {
                        return Ok(ToolResult::error(format!(
                            "Index {idx} out of bounds (have {} hooks)",
                            config.lifecycle_hooks.len()
                        )));
                    }
                    config.lifecycle_hooks.remove(idx)
                } else {
                    let event_str = input["event"].as_str().unwrap_or("");
                    let name = input["name"].as_str().unwrap_or("");

                    if event_str.is_empty() || name.is_empty() {
                        return Ok(ToolResult::error(
                            "Provide either 'index' or both 'event' and 'name' to identify the hook to remove.",
                        ));
                    }

                    let event = match parse_event(event_str) {
                        Some(e) => e,
                        None => {
                            return Ok(ToolResult::error(format!("Invalid event '{event_str}'.")));
                        }
                    };

                    let pos = config
                        .lifecycle_hooks
                        .iter()
                        .position(|h| h.event == event && h.display_name() == name);

                    match pos {
                        Some(i) => config.lifecycle_hooks.remove(i),
                        None => {
                            return Ok(ToolResult::error(format!(
                                "No hook found for event={event_str}, name={name}"
                            )));
                        }
                    }
                };

                if let Err(e) = save_project_config(&root, &config) {
                    return Ok(ToolResult::error(e));
                }

                Ok(ToolResult::text(format!(
                    "✓ Removed hook: {} on {}\n\nRemaining hooks: {}",
                    removed.display_name(),
                    removed.event,
                    config.lifecycle_hooks.len()
                )))
            })
        },
    )
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Parse an event string like `"on_enter(research)"` or `"on_gate_pass"` into a `LifecycleEvent`.
pub(crate) fn parse_event(s: &str) -> Option<LifecycleEvent> {
    let s = s.trim();

    // Phase-based events: on_enter(phase) / on_exit(phase)
    if let Some(inner) = s
        .strip_prefix("on_enter(")
        .and_then(|r| r.strip_suffix(')'))
    {
        let phase = magistr_engine::WorkflowPhase::from_name(inner)?;
        return Some(LifecycleEvent::OnEnter(phase));
    }
    if let Some(inner) = s.strip_prefix("on_exit(").and_then(|r| r.strip_suffix(')')) {
        let phase = magistr_engine::WorkflowPhase::from_name(inner)?;
        return Some(LifecycleEvent::OnExit(phase));
    }

    // Simple events
    match s {
        "on_iteration_start" => Some(LifecycleEvent::OnIterationStart),
        "on_iteration_end" => Some(LifecycleEvent::OnIterationEnd),
        "on_gate_pass" => Some(LifecycleEvent::OnGatePass),
        "on_gate_fail" => Some(LifecycleEvent::OnGateFail),
        _ => None,
    }
}

/// Format hooks into a readable grouped listing.
fn format_hooks_list(hooks: &[LifecycleHook]) -> String {
    // Group by event, preserving declaration order.
    let mut groups: Vec<(&LifecycleEvent, Vec<(usize, &LifecycleHook)>)> = Vec::new();
    for (i, hook) in hooks.iter().enumerate() {
        if let Some((_, group)) = groups.iter_mut().find(|(e, _)| *e == &hook.event) {
            group.push((i, hook));
        } else {
            groups.push((&hook.event, vec![(i, hook)]));
        }
    }

    let mut out = String::with_capacity(256);
    out.push_str(&format!("Lifecycle Hooks ({} total)\n\n", hooks.len()));

    for (event, hooks_in_group) in &groups {
        out.push_str(&format!("## {event}\n\n"));
        for (idx, hook) in hooks_in_group {
            let kind = if hook.action.is_shell_command() {
                "shell"
            } else {
                "mcp"
            };
            out.push_str(&format!("  [{idx}] [{kind}] {}", hook.display_name()));
            if let Some(ref desc) = hook.description {
                out.push_str(&format!(" — {desc}"));
            }
            if hook.on_failure != OnFailure::Continue {
                out.push_str(&format!(" (on_failure: {:?})", hook.on_failure));
            }
            match &hook.action {
                HookAction::McpTool { arguments, .. } if !arguments.is_null() => {
                    out.push_str(&format!(
                        "\n      args: {}",
                        serde_json::to_string(arguments).unwrap_or_default()
                    ));
                }
                HookAction::ShellCommand {
                    timeout_secs: Some(t),
                    ..
                } => {
                    out.push_str(&format!("\n      timeout: {t}s"));
                }
                _ => {}
            }
            out.push('\n');
        }
        out.push('\n');
    }

    out
}

/// Load `ProjectConfig` from `.magistr/project.json`.
fn load_project_config(project_root: &std::path::Path) -> Result<ProjectConfig, String> {
    crate::validate::load_project_config(project_root).map_err(|error| error.to_string())
}

/// Save `ProjectConfig` back to `.magistr/project.json` (hot-reload).
fn save_project_config(
    project_root: &std::path::Path,
    config: &ProjectConfig,
) -> Result<(), String> {
    let config_path =
        crate::paths::project_json(project_root).map_err(|error| error.to_string())?;
    let content = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize config: {e}"))?;
    std::fs::write(&config_path, content)
        .map_err(|e| format!("Failed to write config to {}: {e}", config_path.display()))
}

#[cfg(test)]
#[path = "hooks.tests.rs"]
mod tests;
