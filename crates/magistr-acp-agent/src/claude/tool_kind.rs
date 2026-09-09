//! Claude Code's tool names, as ACP tool kinds.

use agent_client_protocol::schema::v1::ToolKind;

/// The kind a client renders a tool call as, from the tool's name. MCP tools
/// (`mcp__server__tool`) and anything unknown are [`ToolKind::Other`].
pub(crate) fn tool_kind(name: &str) -> ToolKind {
    match name {
        "Bash" | "PowerShell" | "REPL" | "KillShell" | "BashOutput" => ToolKind::Execute,
        "Read" | "NotebookRead" => ToolKind::Read,
        "Glob" | "Grep" | "LSP" => ToolKind::Search,
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => ToolKind::Edit,
        "WebFetch" | "WebSearch" => ToolKind::Fetch,
        "Agent" | "Task" | "TodoWrite" | "TodoRead" => ToolKind::Think,
        "EnterPlanMode" | "ExitPlanMode" => ToolKind::SwitchMode,
        _ => ToolKind::Other,
    }
}

#[cfg(test)]
#[path = "tool_kind.tests.rs"]
mod tests;
