//! Tool access policy enforcement — filter tool lists based on role permissions.
//!
//! Enforces per-role allowed/denied tool lists at `AgentConfig` construction time.
//! Denied tools always take precedence over allowed tools.

use super::role::SpecializedRole;

/// Apply a role's tool policy to a base tool list.
///
/// - If `role.allowed_tools` is non-empty, only those tools are kept from `base_tools`.
/// - `role.denied_tools` are always removed (takes precedence over allowed).
#[must_use]
pub fn apply_tool_policy(role: &SpecializedRole, base_tools: &[String]) -> Vec<String> {
    base_tools
        .iter()
        .filter(|tool| {
            // If allowlist is non-empty, tool must be in it
            let allowed = role.allowed_tools.is_empty() || role.allowed_tools.contains(tool);
            // Tool must not be in denylist
            let not_denied = !role.denied_tools.contains(tool);
            allowed && not_denied
        })
        .cloned()
        .collect()
}

/// Check if a single tool is allowed by the role's policy.
#[must_use]
pub fn is_tool_allowed(role: &SpecializedRole, tool: &str) -> bool {
    let tool_str = tool.to_string();
    let allowed = role.allowed_tools.is_empty() || role.allowed_tools.contains(&tool_str);
    let not_denied = !role.denied_tools.contains(&tool_str);
    allowed && not_denied
}

#[cfg(test)]
#[path = "tool_policy.tests.rs"]
mod tests;
