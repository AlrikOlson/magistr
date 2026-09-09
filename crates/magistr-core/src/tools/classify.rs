//! Tool classification — single entry point for categorizing tool calls.
//!
//! Combines `ToolKind` (from ACP) with name-based pattern matching
//! to produce a `ToolCategory`. This is the ONE place in the codebase
//! that answers "what workflow category does this tool call serve?"

use crate::ToolKind;
use crate::tools::patterns;

/// Workflow-relevant category of a tool call.
///
/// Used by the workflow tracker to accumulate evidence and by the handler
/// to route tool events. Adding a new tool means updating `patterns.rs`,
/// not adding branches here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCategory {
    /// Web research: web_search, brave_*, firecrawl, context7, etc.
    Research,
    /// Codebase exploration: Read, Glob, Grep, scope_*, cs_*
    Exploration,
    /// File mutation: Write, Edit, Delete, Move
    FileWrite,
    /// Shell command execution: Bash
    Command,
    /// Workflow orchestration: magistr_set_phase, magistr_gate, magistr_format
    Orchestration,
    /// Roadmap mutation: roadmap_complete_chunk, roadmap_add_chunk, signal_promote, etc.
    RoadmapMutation,
    /// Todo tracking: TodoWrite
    Todo,
    /// Agent/task spawn
    AgentSpawn,
    /// Unclassified — no workflow signal
    Unknown,
}

impl ToolCategory {
    /// Classify a tool call into a workflow-relevant category.
    ///
    /// Dispatches on `ToolKind` first (authoritative for ACP), then falls
    /// back to name-based pattern matching for `Other` kind or when kind is
    /// absent (non-ACP backends).
    ///
    /// **Key design decision:** `Fetch` always maps to `Research`. You don't
    /// fetch web pages for codebase exploration. This fixes the bug where ACP
    /// display titles like "Fetch https://docs.rs/..." weren't recognized.
    pub fn classify(kind: Option<ToolKind>, name: &str) -> Self {
        // Kind-first dispatch — authoritative when available.
        if let Some(k) = kind {
            match k {
                ToolKind::Fetch => return Self::Research,

                ToolKind::Search => {
                    return if patterns::ToolPatterns::is_research(name) {
                        Self::Research
                    } else {
                        Self::Exploration
                    };
                }

                ToolKind::Read => return Self::Exploration,
                ToolKind::Edit | ToolKind::Delete | ToolKind::Move => {
                    return Self::FileWrite;
                }
                ToolKind::Execute => return Self::Command,
                ToolKind::Think | ToolKind::SwitchMode => return Self::Unknown,

                ToolKind::Other => {
                    // Check for TodoWrite (has "action" or "todos" fields — but we
                    // can't inspect input here, so we classify by name only).
                    // TodoWrite detection by input fields stays in tool_event_from().
                }
            }
        }

        // Name-based fallback: canonical Claude Code tool names.
        match name {
            "Write" | "Edit" | "MultiEdit" | "NotebookEdit" => return Self::FileWrite,
            "Bash" => return Self::Command,
            "TodoWrite" => return Self::Todo,
            "Agent" | "Task" | "TaskCreate" => return Self::AgentSpawn,
            _ => {}
        }

        // Pattern-based matching from the centralized registry.
        patterns::ToolPatterns::classify_name(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_research_by_kind() {
        assert_eq!(
            ToolCategory::classify(Some(ToolKind::Fetch), "anything"),
            ToolCategory::Research
        );
    }

    #[test]
    fn classify_exploration_by_kind() {
        assert_eq!(
            ToolCategory::classify(Some(ToolKind::Read), "Read"),
            ToolCategory::Exploration
        );
    }

    #[test]
    fn classify_file_write_by_name() {
        assert_eq!(
            ToolCategory::classify(None, "Write"),
            ToolCategory::FileWrite
        );
        assert_eq!(
            ToolCategory::classify(None, "Edit"),
            ToolCategory::FileWrite
        );
    }

    #[test]
    fn classify_command_by_name() {
        assert_eq!(ToolCategory::classify(None, "Bash"), ToolCategory::Command);
    }

    #[test]
    fn classify_todo_by_name() {
        assert_eq!(
            ToolCategory::classify(None, "TodoWrite"),
            ToolCategory::Todo
        );
    }

    #[test]
    fn classify_unknown_fallback() {
        assert_eq!(
            ToolCategory::classify(None, "some_random_tool"),
            ToolCategory::Unknown
        );
    }

    #[test]
    fn classify_research_by_pattern() {
        assert_eq!(
            ToolCategory::classify(None, "brave_web_search"),
            ToolCategory::Research
        );
    }

    #[test]
    fn classify_orchestration_by_pattern() {
        assert_eq!(
            ToolCategory::classify(None, "magistr_set_phase"),
            ToolCategory::Orchestration
        );
    }
}
