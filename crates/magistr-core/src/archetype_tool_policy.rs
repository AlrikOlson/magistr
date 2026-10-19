//! Archetype-scoped tool policies — language-specific tool allow/deny lists.
//!
//! When an agent is assigned to a specific archetype, its tool access is scoped
//! to the tools relevant for that archetype's language/framework. Shared tools
//! (git, file ops, search) remain available to all archetypes.

use crate::role::SpecializedRole;
use crate::types::Archetype;

/// Tools available to ALL archetypes regardless of language.
const SHARED_TOOLS: &[&str] = &[
    "Read",
    "Edit",
    "Write",
    "Bash",
    "mcp__magistr__context",
    "mcp__magistr__search",
    "mcp__magistr__read",
    "mcp__magistr__edit",
    "mcp__magistr__write",
    "mcp__magistr__discover",
    "mcp__magistr__execute",
    "mcp__magistr__plan_work",
    "mcp__magistr__start_work",
    "mcp__magistr__record_progress",
    "mcp__magistr__check_work",
    "mcp__magistr__finish_work",
    "mcp__brave-search__brave_web_search",
];

/// Apply archetype-scoped tool policy to a role.
///
/// Merges the archetype's language-specific denied tools into the role's
/// deny list. Shared tools are always kept available.
pub fn apply_archetype_tool_policy(role: &mut SpecializedRole, archetype: &Archetype) {
    let denied = language_denied_tools(&archetype.language);
    for tool in denied {
        let tool_str = tool.to_string();
        if !role.denied_tools.contains(&tool_str) && !is_shared_tool(&tool) {
            role.denied_tools.push(tool_str);
        }
    }
}

/// Check if a tool is in the shared (always-allowed) set.
#[must_use]
pub fn is_shared_tool(tool: &str) -> bool {
    SHARED_TOOLS.contains(&tool)
}

/// Get language-specific denied tools.
///
/// A Rust agent shouldn't have npm tools; a TypeScript agent shouldn't
/// have cargo tools. This returns tool name patterns that should be denied
/// for agents working in the given language.
#[must_use]
pub fn language_denied_tools(language: &str) -> Vec<String> {
    match language {
        "rust" => vec![
            "npm".into(),
            "npx".into(),
            "node".into(),
            "dotnet".into(),
            "pip".into(),
            "python".into(),
            "go".into(),
            "bundle".into(),
            "gem".into(),
            "composer".into(),
            "mix".into(),
        ],
        "typescript" | "javascript" => vec![
            "cargo".into(),
            "rustc".into(),
            "dotnet".into(),
            "pip".into(),
            "python".into(),
            "go".into(),
            "bundle".into(),
            "gem".into(),
            "composer".into(),
            "mix".into(),
        ],
        "python" => vec![
            "cargo".into(),
            "rustc".into(),
            "npm".into(),
            "npx".into(),
            "node".into(),
            "dotnet".into(),
            "go".into(),
            "bundle".into(),
            "gem".into(),
            "composer".into(),
            "mix".into(),
        ],
        "go" => vec![
            "cargo".into(),
            "rustc".into(),
            "npm".into(),
            "npx".into(),
            "node".into(),
            "dotnet".into(),
            "pip".into(),
            "python".into(),
            "bundle".into(),
            "gem".into(),
            "composer".into(),
            "mix".into(),
        ],
        "csharp" => vec![
            "cargo".into(),
            "rustc".into(),
            "npm".into(),
            "npx".into(),
            "node".into(),
            "pip".into(),
            "python".into(),
            "go".into(),
            "bundle".into(),
            "gem".into(),
            "composer".into(),
            "mix".into(),
        ],
        "java" | "kotlin" => vec![
            "cargo".into(),
            "rustc".into(),
            "npm".into(),
            "npx".into(),
            "node".into(),
            "dotnet".into(),
            "pip".into(),
            "python".into(),
            "go".into(),
            "bundle".into(),
            "gem".into(),
            "composer".into(),
            "mix".into(),
        ],
        "ruby" => vec![
            "cargo".into(),
            "rustc".into(),
            "npm".into(),
            "npx".into(),
            "node".into(),
            "dotnet".into(),
            "pip".into(),
            "python".into(),
            "go".into(),
            "composer".into(),
            "mix".into(),
        ],
        "php" => vec![
            "cargo".into(),
            "rustc".into(),
            "npm".into(),
            "npx".into(),
            "node".into(),
            "dotnet".into(),
            "pip".into(),
            "python".into(),
            "go".into(),
            "bundle".into(),
            "gem".into(),
            "mix".into(),
        ],
        "elixir" => vec![
            "cargo".into(),
            "rustc".into(),
            "npm".into(),
            "npx".into(),
            "node".into(),
            "dotnet".into(),
            "pip".into(),
            "python".into(),
            "go".into(),
            "bundle".into(),
            "gem".into(),
            "composer".into(),
        ],
        // Unknown languages get no denials — permissive by default.
        _ => vec![],
    }
}

#[cfg(test)]
#[path = "archetype_tool_policy.tests.rs"]
mod tests;
