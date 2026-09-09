use serde::{Deserialize, Serialize};

use super::lifecycle_event::LifecycleEvent;

/// Strategy when a lifecycle hook fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OnFailure {
    /// Block the phase transition — the workflow cannot proceed.
    FailFast,
    /// Log the failure and continue — non-blocking.
    #[default]
    Continue,
    /// Inject a warning into the agent prompt — non-blocking but visible.
    Warn,
}

/// What a lifecycle hook actually does when triggered.
///
/// Either invokes an MCP tool (the original behavior) or runs a shell command
/// directly. Uses `#[serde(untagged)]` for backward-compatible JSON: existing
/// hooks with `"tool"` field continue to work, new shell hooks use `"command"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HookAction {
    /// Invoke an MCP tool by name with optional arguments.
    McpTool {
        /// MCP tool name (e.g., `"magistr_format"`, `"scope_search"`).
        tool: String,
        /// Arguments to pass to the MCP tool.
        #[serde(default = "default_args", skip_serializing_if = "is_null")]
        arguments: serde_json::Value,
    },
    /// Run a shell command directly in the project directory.
    ShellCommand {
        /// Shell command to execute (via `sh -c`).
        command: String,
        /// Timeout in seconds. Defaults to 30 if not specified.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout_secs: Option<u64>,
    },
}

impl HookAction {
    /// Display name for logging and UI (tool name or command).
    #[must_use]
    pub fn display_name(&self) -> &str {
        match self {
            Self::McpTool { tool, .. } => tool,
            Self::ShellCommand { command, .. } => command,
        }
    }

    /// Whether this is an MCP tool action.
    #[must_use]
    pub fn is_mcp_tool(&self) -> bool {
        matches!(self, Self::McpTool { .. })
    }

    /// Whether this is a shell command action.
    #[must_use]
    pub fn is_shell_command(&self) -> bool {
        matches!(self, Self::ShellCommand { .. })
    }
}

/// A single lifecycle hook binding an event to an action (MCP tool or shell command).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleHook {
    /// The lifecycle event that triggers this hook.
    pub event: LifecycleEvent,
    /// The action to perform when the event fires.
    #[serde(flatten)]
    pub action: HookAction,
    /// Optional description injected into the agent's system prompt
    /// to explain when/why this hook fires.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// What to do when this hook fails.
    #[serde(default, skip_serializing_if = "is_default_on_failure")]
    pub on_failure: OnFailure,
}

impl LifecycleHook {
    /// Convenience accessor for the display name of this hook's action.
    #[must_use]
    pub fn display_name(&self) -> &str {
        self.action.display_name()
    }
}

fn default_args() -> serde_json::Value {
    serde_json::Value::Null
}

fn is_null(v: &serde_json::Value) -> bool {
    v.is_null()
}

fn is_default_on_failure(v: &OnFailure) -> bool {
    *v == OnFailure::Continue
}

/// A resolved hook ready for dispatch — stripped of the event trigger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedHook {
    /// The action to perform.
    #[serde(flatten)]
    pub action: HookAction,
    /// Optional description for prompt injection.
    pub description: Option<String>,
    /// What to do when this hook fails.
    pub on_failure: OnFailure,
}

impl ResolvedHook {
    /// Convenience accessor for the display name of this hook's action.
    #[must_use]
    pub fn display_name(&self) -> &str {
        self.action.display_name()
    }
}

impl From<&LifecycleHook> for ResolvedHook {
    fn from(hook: &LifecycleHook) -> Self {
        Self {
            action: hook.action.clone(),
            description: hook.description.clone(),
            on_failure: hook.on_failure,
        }
    }
}
