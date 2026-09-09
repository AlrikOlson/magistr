//! `magistr_set_phase` MCP tool — workflow phase transitions.

use std::path::PathBuf;

use magistr_mcp::{SdkMcpTool, ToolResult};
use serde_json::json;
use tracing::debug;

/// Build the `magistr_set_phase` MCP tool for explicit workflow phase transitions.
///
/// The agent calls this tool to announce which workflow phase it is entering.
/// The MCP server returns a confirmation string; the real state update happens
/// in the TUI handler which intercepts this tool call by name.
pub(super) fn build_set_phase_tool(project_root: PathBuf) -> SdkMcpTool {
    SdkMcpTool::new(
        "magistr_set_phase",
        "Announce a workflow phase transition. Call this EVERY TIME you move to a new phase: \
         research → plan → execute → verify → done. \
         This updates the magistr TUI progress indicator. \
         Valid phases: research, plan, execute, verify, done.",
        json!({
            "type": "object",
            "properties": {
                "phase": {
                    "type": "string",
                    "description": "The workflow phase you are entering now.",
                    "enum": ["research", "plan", "execute", "verify", "done"]
                }
            },
            "required": ["phase"]
        }),
        move |input| {
            let root = project_root.clone();
            Box::pin(async move {
                let phase = input
                    .get("phase")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                debug!(tool = "magistr_set_phase", phase, "MCP tool invoked");

                // Validate the phase name
                if crate::workflow::tracker::WorkflowPhase::from_name(phase).is_none() {
                    return Ok(ToolResult::error(format!(
                        "Unknown phase '{phase}'. Valid phases: research, plan, execute, verify, done"
                    )));
                }

                // Research guard: query the TUI's workflow guard socket to check
                // if the phase transition is allowed. The socket is served by the
                // TUI process which has the authoritative tracker state.
                // If no socket is available (non-workflow mode), allow the transition.
                if let Some(guard_result) =
                    crate::orchestration::workflow::workflow_socket::query_guard(&root, phase)
                    && !guard_result.allowed
                    && let Some(reason) = guard_result.reason
                {
                    return Ok(ToolResult::error(reason));
                }

                Ok(ToolResult::text(format!("✓ Phase set to: {phase}")))
            })
        },
    )
}
