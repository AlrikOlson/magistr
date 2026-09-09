//! An auto-allow rule that grants permission without prompting.

use serde::{Deserialize, Serialize};

use super::rule_matcher::RuleMatcher;

/// An auto-allow rule that grants permission without prompting.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AutoAllowRule {
    /// Which tool(s) this rule applies to.
    pub matcher: RuleMatcher,
    /// Human-readable description of what this rule allows.
    pub description: String,
}

impl AutoAllowRule {
    /// Create a rule matching a specific tool name.
    pub fn tool_name(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            matcher: RuleMatcher::ToolName { name: name.into() },
            description: description.into(),
        }
    }

    /// Create a rule matching Bash commands with a given prefix.
    pub fn bash_prefix(prefix: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            matcher: RuleMatcher::BashPrefix {
                prefix: prefix.into(),
            },
            description: description.into(),
        }
    }

    /// Create a rule matching Bash commands containing a pattern.
    pub fn bash_contains(pattern: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            matcher: RuleMatcher::BashContains {
                pattern: pattern.into(),
            },
            description: description.into(),
        }
    }

    /// Create a rule matching file operations on paths with a prefix.
    pub fn path_prefix(prefix: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            matcher: RuleMatcher::PathPrefix {
                prefix: prefix.into(),
            },
            description: description.into(),
        }
    }

    /// Create a rule that allows all read operations.
    pub fn all_reads(description: impl Into<String>) -> Self {
        Self {
            matcher: RuleMatcher::AllReads,
            description: description.into(),
        }
    }

    /// Create a rule that allows all operations for a specific tool.
    pub fn all_of_tool(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            matcher: RuleMatcher::AllOfTool { name: name.into() },
            description: description.into(),
        }
    }

    /// Check if this rule matches a given tool call.
    pub fn matches(&self, tool_name: &str, tool_input: &serde_json::Value) -> bool {
        match &self.matcher {
            RuleMatcher::ToolName { name } => tool_name == name,

            RuleMatcher::BashPrefix { prefix } => {
                if tool_name != "Bash" {
                    return false;
                }
                tool_input
                    .get("command")
                    .and_then(|v| v.as_str())
                    .is_some_and(|cmd| cmd.starts_with(prefix.as_str()))
            }

            RuleMatcher::BashContains { pattern } => {
                if tool_name != "Bash" {
                    return false;
                }
                tool_input
                    .get("command")
                    .and_then(|v| v.as_str())
                    .is_some_and(|cmd| cmd.contains(pattern.as_str()))
            }

            RuleMatcher::PathPrefix { prefix } => {
                // Applies to Read, Write, Edit — check file_path field
                let path_field = match tool_name {
                    "Read" | "Write" | "Edit" => "file_path",
                    _ => return false,
                };
                tool_input
                    .get(path_field)
                    .and_then(|v| v.as_str())
                    .is_some_and(|p| p.starts_with(prefix.as_str()))
            }

            RuleMatcher::AllReads => {
                matches!(
                    tool_name,
                    "Read" | "Glob" | "Grep" | "WebSearch" | "WebFetch"
                )
            }

            RuleMatcher::AllOfTool { name } => tool_name == name,
        }
    }
}
