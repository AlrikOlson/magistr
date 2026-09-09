//! Summarize a set of audit entries.

use super::audit_entry::AuditEntry;
use super::audit_summary::AuditSummary;

/// Summarize a set of audit entries.
pub fn summarize(entries: &[AuditEntry]) -> AuditSummary {
    let total = entries.len();
    let allowed = entries.iter().filter(|e| e.allowed).count();
    let denied = total - allowed;

    // Count allowed tools
    let mut tool_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for entry in entries.iter().filter(|e| e.allowed) {
        *tool_counts.entry(entry.tool_name.clone()).or_default() += 1;
    }

    let mut top_allowed_tools: Vec<_> = tool_counts.into_iter().collect();
    top_allowed_tools.sort_by_key(|t| std::cmp::Reverse(t.1));
    top_allowed_tools.truncate(5);

    AuditSummary {
        total,
        allowed,
        denied,
        top_allowed_tools,
    }
}
