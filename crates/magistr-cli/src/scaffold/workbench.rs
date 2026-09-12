//! The scaffold's view of the workspace tools: the launch line is
//! [`McpTarget::Workbench`](crate::mcp_target::McpTarget), this binary's own
//! native bridge; this module pins the vendored routing skill.

/// The workbench-mcp commit whose routing skill is vendored under
/// `templates/skills/workbench-routing`. Bump when re-vendoring.
pub(crate) const ROUTING_SKILL_SOURCE: &str = "workbench-mcp@acab4d5";
