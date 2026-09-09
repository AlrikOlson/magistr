//! Role dispatch — classify subtasks and match them to the best-fit agent role.
//!
//! Uses keyword heuristics on task_id and description to determine which
//! specialized role should handle a given subtask.

use super::role::SpecializedRole;

/// Keyword groups mapped to role IDs.
const RESEARCH_KEYWORDS: &[&str] = &[
    "research",
    "investigate",
    "analyze",
    "survey",
    "explore",
    "discover",
];
const CODER_KEYWORDS: &[&str] = &[
    "implement",
    "code",
    "build",
    "fix",
    "refactor",
    "create",
    "add",
    "develop",
    "write code",
];
const REVIEWER_KEYWORDS: &[&str] = &["review", "critique", "audit", "inspect", "check quality"];
const TESTER_KEYWORDS: &[&str] = &["test", "spec", "verify", "validate", "assert", "coverage"];
const DOCUMENTER_KEYWORDS: &[&str] = &[
    "document",
    "docs",
    "comment",
    "readme",
    "documentation",
    "docstring",
];

/// Classify a subtask to a role ID based on keyword heuristics.
///
/// Returns the role ID string (e.g., "researcher", "coder"). Defaults to "coder"
/// if no keywords match.
#[must_use]
pub fn classify_subtask_role(task_id: &str, description: &str) -> &'static str {
    let combined = format!("{} {}", task_id, description).to_lowercase();

    // Check in priority order: more specific roles first
    if contains_any(&combined, RESEARCH_KEYWORDS) {
        return "researcher";
    }
    if contains_any(&combined, REVIEWER_KEYWORDS) {
        return "reviewer";
    }
    if contains_any(&combined, TESTER_KEYWORDS) {
        return "tester";
    }
    if contains_any(&combined, DOCUMENTER_KEYWORDS) {
        return "documenter";
    }
    if contains_any(&combined, CODER_KEYWORDS) {
        return "coder";
    }

    // Default: coder handles unclassified tasks
    "coder"
}

/// Dispatch a subtask to the best-fit role from available roles.
///
/// Returns the matched role, or falls back to the first available role if the
/// classified role isn't in the available set. Returns `None` if `roles` is empty.
#[must_use]
pub fn dispatch_to_role<'a>(
    task_id: &str,
    description: &str,
    roles: &'a [SpecializedRole],
) -> Option<&'a SpecializedRole> {
    if roles.is_empty() {
        return None;
    }

    let target_id = classify_subtask_role(task_id, description);

    roles
        .iter()
        .find(|r| r.role_id == target_id)
        .or_else(|| roles.first())
}

/// Check if text contains any of the given keywords.
fn contains_any(text: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|kw| text.contains(kw))
}

#[cfg(test)]
#[path = "role_dispatch.tests.rs"]
mod tests;
