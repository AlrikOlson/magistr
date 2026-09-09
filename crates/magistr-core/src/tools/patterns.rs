//! Declarative tool name pattern registry — single source of truth.
//!
//! All tool name → category mapping lives here. Adding a new MCP server's
//! tools means adding entries to the arrays on `ToolPatterns` — nowhere else.

use crate::tools::classify::ToolCategory;
use crate::tools::normalize::normalize_tool_name;

/// Declarative tool name pattern registry.
///
/// Each associated constant maps a set of patterns to a `ToolCategory`.
/// Adding a new MCP server's tools means adding entries here.
pub struct ToolPatterns;

impl ToolPatterns {
    /// Research tool name patterns (substring match, case-insensitive).
    pub const RESEARCH: &[&str] = &[
        // Built-in Claude Code tools
        "web_search",
        "websearch",
        "web_fetch",
        "webfetch",
        // Brave Search MCP server
        "brave_web_search",
        "brave_news_search",
        "brave_image_search",
        "brave_video_search",
        "brave_local_search",
        "brave_summarizer",
        // Firecrawl MCP server
        "firecrawl",
        // Context7 library docs MCP server
        "context7",
        // Other research MCP tools
        "perplexity",
        "tavily",
    ];

    /// Exploration tool name patterns (substring match, case-insensitive).
    pub const EXPLORATION: &[&str] = &[
        // magistr-scope / jig-scope MCP tools
        "scope_search",
        "cs_search",
        "scope_grep",
        "cs_grep",
        "scope_read",
        "cs_read",
        "scope_modules",
        "cs_modules",
        "scope_imports",
        "cs_imports",
        "scope_git",
        "cs_git",
        "magistr-scope",
        "codescope",
    ];

    /// Exploration tool names that require exact (case-insensitive) match.
    pub const EXPLORATION_EXACT: &[&str] = &["read", "glob", "grep", "search", "listfiles"];

    /// Magistr orchestration tool suffixes (matched with `ends_with`).
    pub const ORCHESTRATION_SUFFIXES: &[&str] = &[
        "magistr_set_phase",
        "magistr_gate",
        "magistr_format",
        "magistr_delete",
        "magistr_list",
    ];

    /// Roadmap mutation tool suffixes (matched with `ends_with`): the
    /// think-and-ship `roadmap_*` verbs that change chunk state, plus
    /// `signal_promote`, which creates a chunk. Read-only verbs
    /// (`roadmap_status`, `roadmap_next`, `roadmap_get`, `roadmap_export`)
    /// are deliberately absent.
    pub const ROADMAP_MUTATION_SUFFIXES: &[&str] = &[
        "roadmap_add_chunk",
        "roadmap_update_chunk",
        "roadmap_set_status",
        "roadmap_start_chunk",
        "roadmap_complete_chunk",
        "roadmap_obsolete_chunk",
        "roadmap_reprioritize",
        "roadmap_link",
        "roadmap_set_group",
        "roadmap_focus_set",
        "signal_promote",
    ];

    /// Classify a tool name into a workflow category.
    ///
    /// Checks both the original and normalized name for substring patterns,
    /// because MCP prefixes like `mcp__firecrawl__scrape` contain the server
    /// name which is itself a pattern match (e.g., "firecrawl").
    pub fn classify_name(name: &str) -> ToolCategory {
        let original_lower = name.to_lowercase();
        let normalized = normalize_tool_name(name);
        let normalized_lower = normalized.to_lowercase();

        // Research: substring match (check both original and normalized)
        if Self::RESEARCH
            .iter()
            .any(|p| original_lower.contains(p) || normalized_lower.contains(p))
        {
            return ToolCategory::Research;
        }

        // Exploration: substring match (check both original and normalized)
        if Self::EXPLORATION
            .iter()
            .any(|p| original_lower.contains(p) || normalized_lower.contains(p))
        {
            return ToolCategory::Exploration;
        }

        // Exploration: exact match on normalized name
        if Self::EXPLORATION_EXACT
            .iter()
            .any(|p| normalized_lower == *p)
        {
            return ToolCategory::Exploration;
        }

        // Orchestration: suffix match on normalized name
        if Self::ORCHESTRATION_SUFFIXES
            .iter()
            .any(|s| normalized_lower.ends_with(s))
        {
            return ToolCategory::Orchestration;
        }

        // Roadmap mutation: suffix match on normalized name
        if Self::ROADMAP_MUTATION_SUFFIXES
            .iter()
            .any(|s| normalized_lower.ends_with(s))
        {
            return ToolCategory::RoadmapMutation;
        }

        ToolCategory::Unknown
    }

    /// Check if a tool name is the `magistr_set_phase` tool specifically.
    pub fn is_set_phase(name: &str) -> bool {
        let normalized = normalize_tool_name(name);
        let lower = normalized.to_lowercase();
        lower.ends_with("magistr_set_phase")
    }

    /// Check if a tool name is a roadmap-mutating tool.
    pub fn is_roadmap_mutation(name: &str) -> bool {
        let normalized = normalize_tool_name(name);
        let lower = normalized.to_lowercase();
        Self::ROADMAP_MUTATION_SUFFIXES
            .iter()
            .any(|s| lower.ends_with(s))
    }

    /// Check if a tool name matches a research tool pattern.
    pub fn is_research(name: &str) -> bool {
        let original_lower = name.to_lowercase();
        let normalized = normalize_tool_name(name);
        let normalized_lower = normalized.to_lowercase();
        Self::RESEARCH
            .iter()
            .any(|p| original_lower.contains(p) || normalized_lower.contains(p))
    }

    /// Check if a tool name matches an exploration tool pattern.
    pub fn is_exploration(name: &str) -> bool {
        let original_lower = name.to_lowercase();
        let normalized = normalize_tool_name(name);
        let normalized_lower = normalized.to_lowercase();
        Self::EXPLORATION
            .iter()
            .any(|p| original_lower.contains(p) || normalized_lower.contains(p))
            || Self::EXPLORATION_EXACT
                .iter()
                .any(|p| normalized_lower == *p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn research_patterns() {
        assert!(ToolPatterns::is_research("web_search"));
        assert!(ToolPatterns::is_research("brave_web_search"));
        assert!(ToolPatterns::is_research("firecrawl_scrape"));
        assert!(ToolPatterns::is_research("context7"));
        assert!(ToolPatterns::is_research(
            "mcp__brave-search__brave_web_search"
        ));
        assert!(!ToolPatterns::is_research("Read"));
        assert!(!ToolPatterns::is_research("scope_search"));
    }

    #[test]
    fn exploration_patterns() {
        assert!(ToolPatterns::is_exploration("scope_search"));
        assert!(ToolPatterns::is_exploration("cs_grep"));
        assert!(ToolPatterns::is_exploration("mcp__jig-scope__scope_search"));
        assert!(!ToolPatterns::is_exploration("web_search"));
    }

    #[test]
    fn orchestration_classification() {
        assert_eq!(
            ToolPatterns::classify_name("magistr_set_phase"),
            ToolCategory::Orchestration
        );
        assert_eq!(
            ToolPatterns::classify_name("magistr_gate"),
            ToolCategory::Orchestration
        );
    }

    #[test]
    fn roadmap_mutation_classification() {
        assert_eq!(
            ToolPatterns::classify_name("roadmap_complete_chunk"),
            ToolCategory::RoadmapMutation
        );
        assert_eq!(
            ToolPatterns::classify_name("mcp__magistr__signal_promote"),
            ToolCategory::RoadmapMutation
        );
        assert_ne!(
            ToolPatterns::classify_name("roadmap_status"),
            ToolCategory::RoadmapMutation
        );
    }

    #[test]
    fn is_set_phase() {
        assert!(ToolPatterns::is_set_phase("magistr_set_phase"));
        assert!(ToolPatterns::is_set_phase(
            "mcp__magistr__magistr_set_phase"
        ));
        assert!(!ToolPatterns::is_set_phase("magistr_gate"));
    }
}
