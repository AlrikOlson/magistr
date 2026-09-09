//! The scaffold's view of workbench-mcp: the resolver lives in
//! [`magistr_core::workbench_command`] (shared with the GUI sandbox); this
//! module pins the vendored routing skill.

pub use magistr_core::workbench_command::WorkbenchCommand;

/// The workbench-mcp commit whose routing skill is vendored under
/// `templates/skills/workbench-routing`. Bump when re-vendoring.
pub(crate) const ROUTING_SKILL_SOURCE: &str = "workbench-mcp@acab4d5";
