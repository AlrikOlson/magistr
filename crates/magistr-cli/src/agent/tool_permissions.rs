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

/// Public tools on the unified connection. Advanced effects are authorized
/// individually by the native catalog, including the purpose ceiling.
const UNIFIED_MCP: &[&str] = &[
    "context",
    "search",
    "read",
    "edit",
    "write",
    "plan_work",
    "start_work",
    "record_progress",
    "check_work",
    "finish_work",
    "discover",
    "execute",
];
const INSPECTION_MCP: &[&str] = &["context", "search", "read", "discover"];

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
        Self::base()
            .with(CODEBASE_READ)
            .with(&["Bash"])
            .with(INSPECTION_MCP)
    }

    /// Roadmap generation context: base + read-only codebase access.
    ///
    /// The agent authors the roadmap exclusively through the think-and-ship
    /// `roadmap_*` tools (`roadmap_add_chunk`), which are pre-approved too.
    #[must_use]
    pub fn for_roadmap() -> Self {
        Self::base()
            .with(CODEBASE_READ)
            .with(&["context", "search", "read", "discover", "execute"])
    }

    /// Planning requests may inspect sources and update the native plan, without
    /// implementation, completion, export, shell, or source-write tools.
    #[must_use]
    pub fn for_roadmap_request() -> Self {
        Self::base().with(CODEBASE_READ).with(&[
            "WebSearch",
            "WebFetch",
            "context",
            "search",
            "read",
            "discover",
            "execute",
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
            .with(UNIFIED_MCP)
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
