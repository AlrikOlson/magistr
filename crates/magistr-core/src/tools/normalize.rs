//! Tool name normalization — single source of truth.
//!
//! ACP reports MCP tool calls in several formats depending on the backend:
//! - `mcp__<server>__<tool>` (Claude Code / some ACP variants)
//! - `<server>-<tool>` (ACP MCP tool format, e.g. `magistr-magistr_recommend`)
//!
//! This module provides a single function to strip the prefix and return
//! the bare tool name.

use std::borrow::Cow;

/// Strip MCP/ACP server prefixes from a tool name.
///
/// Returns the bare tool name, borrowing from the input when no prefix
/// is present (zero-copy fast path).
///
/// # Examples
/// ```
/// # use magistr_core::tools::normalize::normalize_tool_name;
/// assert_eq!(normalize_tool_name("mcp__jig__magistr_recommend"), "magistr_recommend");
/// assert_eq!(normalize_tool_name("magistr-magistr_recommend"), "magistr_recommend");
/// assert_eq!(normalize_tool_name("WebSearch"), "WebSearch");
/// ```
pub fn normalize_tool_name(name: &str) -> Cow<'_, str> {
    // "mcp__jig__magistr_recommend" → "magistr_recommend"
    if let Some(rest) = name.strip_prefix("mcp__")
        && let Some((_server, tool)) = rest.split_once("__")
    {
        return Cow::Owned(tool.to_string());
    }
    // "magistr-magistr_recommend" → "magistr_recommend" (ACP dash-separated format)
    // Only apply when the name has no spaces — MCP tool names are identifiers,
    // while freeform ACP titles (e.g. "grep -A2 schemars") contain spaces.
    if !name.contains(' ')
        && let Some((_server, tool)) = name.split_once('-')
    {
        return Cow::Owned(tool.to_string());
    }
    Cow::Borrowed(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_mcp_prefix() {
        assert_eq!(
            normalize_tool_name("mcp__jig__magistr_recommend"),
            "magistr_recommend"
        );
    }

    #[test]
    fn strip_acp_dash_prefix() {
        assert_eq!(
            normalize_tool_name("magistr-magistr_recommend"),
            "magistr_recommend"
        );
    }

    #[test]
    fn no_prefix_passthrough() {
        assert_eq!(normalize_tool_name("WebSearch"), "WebSearch");
    }

    #[test]
    fn freeform_title_with_dash_not_stripped() {
        // ACP freeform titles contain spaces — dash-split must not fire
        assert_eq!(
            normalize_tool_name("grep -A2 \"schemars\" /path/to/file"),
            "grep -A2 \"schemars\" /path/to/file"
        );
        assert_eq!(
            normalize_tool_name("cat /workspace/example/Cargo.toml"),
            "cat /workspace/example/Cargo.toml"
        );
    }
}
