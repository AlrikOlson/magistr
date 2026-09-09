//! What an auto-allow rule matches against.

use serde::{Deserialize, Serialize};

/// What a rule matches against.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuleMatcher {
    /// Match a specific tool by exact name.
    ToolName { name: String },
    /// Match Bash commands that start with a given prefix.
    BashPrefix { prefix: String },
    /// Match Bash commands containing a substring.
    BashContains { pattern: String },
    /// Match file operations (Read/Write/Edit) on paths with a given prefix.
    PathPrefix { prefix: String },
    /// Match all reads in the current project.
    AllReads,
    /// Match all operations for a specific tool (any input).
    AllOfTool { name: String },
}
