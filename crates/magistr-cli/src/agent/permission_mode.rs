//! How much the agent may do without asking.

use serde::{Deserialize, Serialize};

/// Permission mode for tool execution, as the agent understands it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    /// The agent prompts for dangerous tools.
    Default,
    /// File edits are accepted without a prompt; everything else asks.
    AcceptEdits,
    /// Plan only — no edits.
    Plan,
    /// Every tool is allowed. Skips the agent's hook pipeline; use with care.
    BypassPermissions,
}
