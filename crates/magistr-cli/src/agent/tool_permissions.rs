/// Composable tool permission sets for agent sessions.
///
/// Replaces the previous `DEFAULT_ALLOWED_TOOLS`, `INTERVIEW_ALLOWED_TOOLS`,
/// `ROADMAP_ALLOWED_TOOLS`, and `WORKFLOW_ALLOWED_TOOLS` const arrays with a
/// single builder that composes a base set with context-specific extensions.
#[derive(Debug, Clone)]
pub struct ToolPermissions {
    tools: Vec<&'static str>,
}

/// Base tools pre-approved for every agent session.
const BASE: &[&str] = &["web_search"];

/// Read-only codebase exploration tools.
const CODEBASE_READ: &[&str] = &["Read", "Glob", "Grep"];

/// Full development tools for autonomous workflow iterations.
///
/// Write and Edit are intentionally excluded — they flow through the
/// permission handler so we can block writes to protected files (e.g. ROADMAP.md).
const DEV_TOOLS: &[&str] = &[
    "Bash",
    "WebSearch",
    "WebFetch",
    "NotebookEdit",
    "TodoRead",
    "TodoWrite",
];

/// think-and-ship tools the workflow loop calls, served by the same `magistr`
/// MCP server: the roadmap (plan-of-plans), the reasoning trace, the
/// execution trace and stakeholder signals.
const THINK_AND_SHIP_MCP: &[&str] = &[
    "roadmap_status",
    "roadmap_next",
    "roadmap_get",
    "roadmap_start_chunk",
    "roadmap_complete_chunk",
    "roadmap_add_chunk",
    "roadmap_update_chunk",
    "roadmap_set_status",
    "roadmap_obsolete_chunk",
    "roadmap_reprioritize",
    "roadmap_link",
    "roadmap_export",
    "think_record_step",
    "think_engine_status",
    "think_pin_step",
    "think_trace_checkpoint",
    "think_search_trace",
    "think_get_step",
    "think_set_branch_status",
    "think_step_impact",
    "think_revise_estimate",
    "ship_set_objective",
    "ship_plan",
    "ship_start",
    "ship_record",
    "ship_complete",
    "ship_block",
    "ship_check",
    "ship_finalize",
    "ship_status",
    "signal_capture",
    "signal_pending",
];

/// The workbench-mcp tools (`.claude/rules/tools.md`): the workflow reads
/// and edits the tree through them, so they are pre-approved like the
/// built-in dev tools they replace.
const WORKBENCH_MCP: &[&str] = &[
    "search",
    "map",
    "read",
    "edit",
    "files",
    "write",
    "run",
    "script",
    "undo",
    "journal",
    "workbench_status",
];

/// MCP tools for workflow orchestration.
const WORKFLOW_MCP: &[&str] = &[
    "magistr_set_phase",
    "magistr_gate",
    "magistr_format",
    "magistr_delete",
    "magistr_list",
];

impl ToolPermissions {
    /// Base permission set: only `web_search`.
    ///
    /// Used as the default for `AgentConfig::new()`.
    #[must_use]
    pub fn base() -> Self {
        Self {
            tools: BASE.to_vec(),
        }
    }

    /// Interview context: base + read-only codebase access + Bash.
    ///
    /// Gives the AI read-only access to explore the project during `magistr init`.
    #[must_use]
    pub fn for_interview() -> Self {
        Self::base().with(CODEBASE_READ).with(&["Bash"])
    }

    /// Roadmap generation context: base + read-only codebase access.
    ///
    /// The agent authors the roadmap exclusively through the think-and-ship
    /// `roadmap_*` tools (`roadmap_add_chunk`), which are pre-approved too.
    #[must_use]
    pub fn for_roadmap() -> Self {
        Self::base().with(CODEBASE_READ).with(THINK_AND_SHIP_MCP)
    }

    /// Planning requests may inspect sources and update the native plan, without
    /// implementation, completion, export, shell, or source-write tools.
    #[must_use]
    pub fn for_roadmap_request() -> Self {
        Self::base().with(CODEBASE_READ).with(&[
            "WebSearch",
            "WebFetch",
            "search",
            "map",
            "read",
            "workbench_status",
            "roadmap_status",
            "roadmap_next",
            "roadmap_get",
            "roadmap_add_chunk",
            "roadmap_update_chunk",
            "roadmap_obsolete_chunk",
            "roadmap_reprioritize",
            "roadmap_link",
            "think_record_step",
            "think_engine_status",
            "think_search_trace",
            "think_get_step",
            "think_set_branch_status",
            "think_step_impact",
            "think_revise_estimate",
            "ship_status",
            "signal_pending",
        ])
    }

    /// Workflow iteration context: base + full dev tools + MCP tools.
    ///
    /// Pre-approving at the ACP level eliminates interactive permission prompts
    /// for the vast majority of tool calls.
    #[must_use]
    pub fn for_workflow() -> Self {
        Self::base()
            .with(CODEBASE_READ)
            .with(DEV_TOOLS)
            .with(THINK_AND_SHIP_MCP)
            .with(WORKFLOW_MCP)
            .with(WORKBENCH_MCP)
    }

    /// Extend with additional tools, deduplicating.
    #[must_use]
    pub fn with(mut self, extra: &[&'static str]) -> Self {
        for tool in extra {
            if !self.tools.contains(tool) {
                self.tools.push(tool);
            }
        }
        self
    }

    /// Convert to owned `String` vec for `AgentConfig::allowed_tools`.
    #[must_use]
    pub fn into_strings(self) -> Vec<String> {
        self.tools.iter().map(|s| (*s).to_string()).collect()
    }

    /// Check whether a tool name is in this permission set.
    #[must_use]
    pub fn contains(&self, tool: &str) -> bool {
        self.tools.contains(&tool)
    }

    /// The tool names as a slice.
    #[must_use]
    pub fn as_slice(&self) -> &[&'static str] {
        &self.tools
    }
}

#[cfg(test)]
#[path = "tool_permissions.tests.rs"]
mod tests;
