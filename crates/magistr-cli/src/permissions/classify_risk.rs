//! Risk classification for tool calls.

use super::high_risk_bash_patterns::HIGH_RISK_BASH_PATTERNS;
use super::risk_level::RiskLevel;

/// Known low-risk (read-only) tool names.
const LOW_RISK_TOOLS: &[&str] = &[
    "Read",
    "Glob",
    "Grep",
    "WebSearch",
    "WebFetch",
    "ListMcpResourcesTool",
    "ReadMcpResourceTool",
    "TodoRead",
];

/// Known medium-risk (file modification) tool names.
const MEDIUM_RISK_TOOLS: &[&str] = &["Write", "Edit", "NotebookEdit", "TodoWrite"];

/// Classify the risk level of a tool call.
///
/// Uses the tool name and input parameters to determine risk.
/// Bash commands are further analyzed for destructive patterns.
pub fn classify_risk(tool_name: &str, tool_input: &serde_json::Value) -> RiskLevel {
    // Check for known low-risk tools
    if LOW_RISK_TOOLS.contains(&tool_name) {
        return RiskLevel::Low;
    }

    // Check for known medium-risk tools
    if MEDIUM_RISK_TOOLS.contains(&tool_name) {
        return RiskLevel::Medium;
    }

    // Bash commands need deeper analysis
    if tool_name == "Bash" {
        return classify_bash_risk(tool_input);
    }

    // MCP tools — default to medium (they can do arbitrary things)
    if tool_name.starts_with("mcp__") {
        return RiskLevel::Medium;
    }

    // Unknown tools — default to high (be conservative)
    RiskLevel::High
}

/// Classify risk for a Bash tool call based on the command content.
fn classify_bash_risk(tool_input: &serde_json::Value) -> RiskLevel {
    let Some(command) = tool_input
        .get("command")
        .and_then(serde_json::Value::as_str)
        .filter(|command| !command.trim().is_empty())
    else {
        // Unknown effects cannot establish read-only or reversible behavior.
        return RiskLevel::High;
    };

    // Check for high-risk patterns
    for pattern in HIGH_RISK_BASH_PATTERNS {
        if command.contains(pattern) {
            return RiskLevel::High;
        }
    }

    // Common safe commands → low risk
    let safe_prefixes = [
        "ls",
        "cat",
        "echo",
        "pwd",
        "which",
        "whoami",
        "date",
        "env",
        "printenv",
        "head",
        "tail",
        "wc",
        "sort",
        "uniq",
        "grep",
        "find",
        "tree",
        "file",
        "stat",
        "du",
        "df",
        "git status",
        "git log",
        "git diff",
        "git show",
        "git branch",
        "cargo check",
        "cargo test",
        "cargo clippy",
        "cargo build",
        "cargo fmt",
        "npm test",
        "npm run",
        "npx ",
        "node ",
        "python -m pytest",
        "pytest",
        "ruff",
        "mypy",
        "go test",
        "go build",
        "go vet",
    ];

    for prefix in &safe_prefixes {
        if command.starts_with(prefix) {
            return RiskLevel::Low;
        }
    }

    // Default: medium risk for unknown bash commands
    RiskLevel::Medium
}

#[cfg(test)]
#[path = "classify_risk.tests.rs"]
mod tests;
