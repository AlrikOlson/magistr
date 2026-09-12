//! `magistr_set_phase` MCP tool — workflow phase transitions.

use magistr_engine::WorkflowPhase;
use magistr_mcp::{SdkMcpTool, ToolResult};
use serde_json::json;
use tracing::debug;

/// Build the `magistr_set_phase` MCP tool for explicit workflow phase transitions.
///
/// The agent calls this tool to announce which workflow phase it is entering.
/// The tool validates the name against the domain's phases and answers a
/// confirmation; the service that serves it guards the transition against
/// the ship plan and announces the phase typed on the feed.
pub(super) fn build_set_phase_tool() -> SdkMcpTool {
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
        |input| {
            Box::pin(async move {
                let phase = input
                    .get("phase")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                debug!(tool = "magistr_set_phase", phase, "MCP tool invoked");

                if WorkflowPhase::from_name(phase).is_none() {
                    return Ok(ToolResult::error(format!(
                        "Unknown phase '{phase}'. Valid phases: research, plan, execute, verify, done"
                    )));
                }

                Ok(ToolResult::text(format!("✓ Phase set to: {phase}")))
            })
        },
    )
}
