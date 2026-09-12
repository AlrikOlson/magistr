//! MCP results report logical failures as errors. Clients isolate dependent work.
use rmcp::model::CallToolResult;
use serde_json::Value;

/// A successful result carrying the same JSON in structured and text content.
#[must_use]
pub fn ok(value: Value) -> CallToolResult {
    CallToolResult::structured(value)
}

/// Compatibility name for existing callers. A legacy string error carries no
/// proof of rollback, so its effect state is conservatively unknown.
#[must_use]
pub fn soft_error(error_kind: &str, message: impl Into<String>) -> CallToolResult {
    super::tool_failure::ToolFailure::uncertain(error_kind, message)
}
