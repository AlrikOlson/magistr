//! Context health tracking and analysis.
//!
//! Monitors context window usage across an agent session, providing:
//! - **Health indicator** — percentage of context window used
//! - **Breakdown** — token usage by category (system prompt, conversation, files, tools)
//! - **Warnings** — alerts when context is getting full
//! - **Compaction suggestions** — actionable advice to reduce context size

#![allow(dead_code)] // Public API — wired from TUI sidebar and agent session handler

use std::fmt;

/// Default context window size in tokens (Claude 3.5/4 default).
pub const DEFAULT_CONTEXT_WINDOW: u64 = 200_000;

/// Warning threshold percentages.
const THRESHOLD_INFO: f64 = 0.70;
const THRESHOLD_WARNING: f64 = 0.85;
const THRESHOLD_CRITICAL: f64 = 0.95;

/// Categories of context window usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextCategory {
    /// System prompt and project instructions.
    SystemPrompt,
    /// Conversation history (user + assistant messages).
    ConversationHistory,
    /// File contents loaded into context.
    FileContext,
    /// Tool call results (search, grep, read outputs).
    ToolResults,
}

impl ContextCategory {
    /// All categories in display order.
    pub const ALL: &'static [Self] = &[
        Self::SystemPrompt,
        Self::ConversationHistory,
        Self::FileContext,
        Self::ToolResults,
    ];
}

impl fmt::Display for ContextCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SystemPrompt => write!(f, "System Prompt"),
            Self::ConversationHistory => write!(f, "Conversation"),
            Self::FileContext => write!(f, "File Context"),
            Self::ToolResults => write!(f, "Tool Results"),
        }
    }
}

/// Severity level for context health warnings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WarningSeverity {
    /// Context usage above 70% — informational.
    Info,
    /// Context usage above 85% — should consider new session.
    Warning,
    /// Context usage above 95% — immediate action needed.
    Critical,
}

impl fmt::Display for WarningSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Warning => write!(f, "warning"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// A warning about context window usage.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextWarning {
    /// Severity of the warning.
    pub severity: WarningSeverity,
    /// Human-readable message.
    pub message: String,
}

/// Token usage broken down by category.
#[derive(Debug, Clone, PartialEq)]
pub struct CategoryUsage {
    /// The category.
    pub category: ContextCategory,
    /// Estimated token count for this category.
    pub tokens: u64,
}

/// A suggestion for reducing context size.
#[derive(Debug, Clone, PartialEq)]
pub struct CompactionSuggestion {
    /// Short description of what can be compacted.
    pub description: String,
    /// Estimated tokens that would be saved.
    pub estimated_savings: u64,
    /// Number of items that could be compacted (e.g., "3 large tool results").
    pub item_count: u32,
}

/// A single tool result tracked for compaction analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackedToolResult {
    /// Tool name (e.g., "Read", "Bash", "scope_search").
    pub tool_name: String,
    /// Estimated token count of the result.
    pub tokens: u64,
    /// Number of turns since this result was last referenced.
    pub turns_since_referenced: u32,
}

/// Context health state for a session.
///
/// Tracks token usage across categories and the context window limit.
/// Updated after each agent turn with new usage data.
#[derive(Debug, Clone)]
pub struct ContextHealth {
    /// Maximum context window size in tokens.
    context_window: u64,
    /// Token usage per category.
    system_prompt_tokens: u64,
    conversation_tokens: u64,
    file_context_tokens: u64,
    tool_results_tokens: u64,
    /// Tracked tool results for compaction suggestions.
    tool_results: Vec<TrackedToolResult>,
    /// Whether a warning has been issued at each severity level.
    warned_info: bool,
    warned_warning: bool,
    warned_critical: bool,
}

impl ContextHealth {
    /// Create a new context health tracker with the given window size.
    pub fn new(context_window: u64) -> Self {
        Self {
            context_window,
            system_prompt_tokens: 0,
            conversation_tokens: 0,
            file_context_tokens: 0,
            tool_results_tokens: 0,
            tool_results: Vec::new(),
            warned_info: false,
            warned_warning: false,
            warned_critical: false,
        }
    }

    /// Total tokens currently in context.
    pub fn total_tokens(&self) -> u64 {
        self.system_prompt_tokens
            + self.conversation_tokens
            + self.file_context_tokens
            + self.tool_results_tokens
    }

    /// Context window size.
    pub fn context_window(&self) -> u64 {
        self.context_window
    }

    /// Usage as a fraction (0.0 to 1.0+).
    pub fn usage_fraction(&self) -> f64 {
        if self.context_window == 0 {
            return 0.0;
        }
        self.total_tokens() as f64 / self.context_window as f64
    }

    /// Usage as a percentage (0 to 100+).
    pub fn usage_percent(&self) -> f64 {
        self.usage_fraction() * 100.0
    }

    /// Human-readable usage string like "42% used (38K/90K tokens)".
    #[must_use]
    pub fn usage_display(&self) -> String {
        let total = self.total_tokens();
        let window = self.context_window;
        let pct = self.usage_percent();
        format!(
            "{pct:.0}% used ({}/{})",
            format_tokens(total),
            format_tokens(window)
        )
    }

    /// Get the breakdown of usage by category.
    pub fn breakdown(&self) -> Vec<CategoryUsage> {
        vec![
            CategoryUsage {
                category: ContextCategory::SystemPrompt,
                tokens: self.system_prompt_tokens,
            },
            CategoryUsage {
                category: ContextCategory::ConversationHistory,
                tokens: self.conversation_tokens,
            },
            CategoryUsage {
                category: ContextCategory::FileContext,
                tokens: self.file_context_tokens,
            },
            CategoryUsage {
                category: ContextCategory::ToolResults,
                tokens: self.tool_results_tokens,
            },
        ]
    }

    /// Update system prompt token count.
    pub fn set_system_prompt_tokens(&mut self, tokens: u64) {
        self.system_prompt_tokens = tokens;
    }

    /// Update conversation history token count.
    pub fn set_conversation_tokens(&mut self, tokens: u64) {
        self.conversation_tokens = tokens;
    }

    /// Update file context token count.
    pub fn set_file_context_tokens(&mut self, tokens: u64) {
        self.file_context_tokens = tokens;
    }

    /// Update tool results token count.
    pub fn set_tool_results_tokens(&mut self, tokens: u64) {
        self.tool_results_tokens = tokens;
    }

    /// Record a tool result for compaction tracking.
    pub fn record_tool_result(&mut self, tool_name: impl Into<String>, tokens: u64) {
        self.tool_results.push(TrackedToolResult {
            tool_name: tool_name.into(),
            tokens,
            turns_since_referenced: 0,
        });
    }

    /// Advance all tracked tool results by one turn (for staleness tracking).
    pub fn advance_turn(&mut self) {
        for result in &mut self.tool_results {
            result.turns_since_referenced += 1;
        }
    }

    /// Mark a tool result as recently referenced (reset its staleness counter).
    pub fn mark_referenced(&mut self, tool_name: &str) {
        for result in &mut self.tool_results {
            if result.tool_name == tool_name {
                result.turns_since_referenced = 0;
            }
        }
    }

    /// Check for warnings based on current usage.
    ///
    /// Returns a warning only for the highest severity that hasn't been
    /// issued yet. Once a severity is issued, it won't be returned again.
    pub fn check_warning(&mut self) -> Option<ContextWarning> {
        let fraction = self.usage_fraction();

        if fraction >= THRESHOLD_CRITICAL && !self.warned_critical {
            self.warned_critical = true;
            self.warned_warning = true;
            self.warned_info = true;
            return Some(ContextWarning {
                severity: WarningSeverity::Critical,
                message: format!(
                    "Context {:.0}% full — start a new session to avoid degraded performance",
                    fraction * 100.0
                ),
            });
        }

        if fraction >= THRESHOLD_WARNING && !self.warned_warning {
            self.warned_warning = true;
            self.warned_info = true;
            return Some(ContextWarning {
                severity: WarningSeverity::Warning,
                message: format!(
                    "Context {:.0}% — consider starting a new session",
                    fraction * 100.0
                ),
            });
        }

        if fraction >= THRESHOLD_INFO && !self.warned_info {
            self.warned_info = true;
            return Some(ContextWarning {
                severity: WarningSeverity::Info,
                message: format!(
                    "Context {:.0}% used — {} remaining",
                    fraction * 100.0,
                    format_tokens(self.context_window.saturating_sub(self.total_tokens()))
                ),
            });
        }

        None
    }

    /// Generate compaction suggestions based on tracked tool results.
    ///
    /// Suggests compacting tool results that are:
    /// - Large (above `min_tokens` threshold)
    /// - Stale (not referenced in `stale_turns` or more turns)
    pub fn compaction_suggestions(
        &self,
        min_tokens: u64,
        stale_turns: u32,
    ) -> Vec<CompactionSuggestion> {
        let mut suggestions = Vec::new();

        // Find large, stale tool results
        let stale_results: Vec<&TrackedToolResult> = self
            .tool_results
            .iter()
            .filter(|r| r.tokens >= min_tokens && r.turns_since_referenced >= stale_turns)
            .collect();

        if !stale_results.is_empty() {
            let total_savings: u64 = stale_results.iter().map(|r| r.tokens).sum();
            let count = stale_results.len() as u32;

            suggestions.push(CompactionSuggestion {
                description: format!(
                    "{count} large tool result{} could be compacted to save {}",
                    if count == 1 { "" } else { "s" },
                    format_tokens(total_savings)
                ),
                estimated_savings: total_savings,
                item_count: count,
            });
        }

        suggestions
    }
}

impl Default for ContextHealth {
    fn default() -> Self {
        Self::new(DEFAULT_CONTEXT_WINDOW)
    }
}

/// Format a token count in human-readable form (e.g., "38K", "1.2M", "500").
fn format_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        let m = tokens as f64 / 1_000_000.0;
        if m >= 10.0 {
            format!("{m:.0}M tokens")
        } else {
            format!("{m:.1}M tokens")
        }
    } else if tokens >= 1_000 {
        let k = tokens as f64 / 1_000.0;
        if k >= 10.0 {
            format!("{k:.0}K tokens")
        } else {
            format!("{k:.1}K tokens")
        }
    } else {
        format!("{tokens} tokens")
    }
}

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
