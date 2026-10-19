//! Role recommendation — suggest optimal role composition based on task type.
//!
//! Analyzes a task description and recommends which agent roles should be
//! involved, enabling the `magistr_recommend` integration to suggest
//! multi-agent compositions.

/// Recommend a set of role IDs for a given task description.
///
/// Returns role IDs sorted by pipeline order (research -> code -> test -> review -> docs).
#[must_use]
pub fn recommend_roles_for_task(task: &str) -> Vec<&'static str> {
    let (roles, _) = recommend_roles_with_description(task);
    roles
}

/// Recommend roles with a human-readable description of the recommendation.
#[must_use]
pub fn recommend_roles_with_description(task: &str) -> (Vec<&'static str>, String) {
    let lower = task.to_lowercase();

    let is_bug_fix = contains_any(&lower, &["fix", "bug", "patch", "hotfix"]);
    let is_feature = contains_any(
        &lower,
        &[
            "implement",
            "add",
            "create",
            "build",
            "new feature",
            "develop",
        ],
    );
    let is_refactor = contains_any(
        &lower,
        &[
            "refactor",
            "restructure",
            "reorganize",
            "clean up",
            "modernize",
        ],
    );
    let is_review = contains_any(&lower, &["review", "audit", "critique", "inspect"]);
    let is_docs = contains_any(
        &lower,
        &["document", "docs", "readme", "documentation", "comment"],
    );
    let is_research = contains_any(
        &lower,
        &[
            "research",
            "investigate",
            "explore",
            "analyze",
            "survey",
            "discover",
        ],
    );
    let is_test = contains_any(&lower, &["test", "spec", "coverage", "verify"]);

    let mut roles = Vec::new();
    let description = if is_review {
        roles.push("reviewer");
        if is_research {
            roles.push("researcher");
        }
        "Code review task: reviewer analyzes the changes."
    } else if is_docs {
        roles.push("documenter");
        if is_research {
            roles.push("researcher");
        }
        "Documentation task: documenter updates docs."
    } else if is_research && !is_feature && !is_bug_fix {
        roles.push("researcher");
        "Research task: researcher investigates and reports findings."
    } else if is_bug_fix {
        roles.push("coder");
        roles.push("tester");
        "Bug fix: coder fixes the issue, tester verifies the fix."
    } else if is_feature {
        roles.push("researcher");
        roles.push("coder");
        roles.push("tester");
        "New feature: researcher gathers context, coder implements, tester validates."
    } else if is_refactor {
        roles.push("coder");
        roles.push("reviewer");
        roles.push("tester");
        "Refactor: coder restructures, reviewer checks quality, tester ensures no regressions."
    } else if is_test {
        roles.push("tester");
        "Testing task: tester writes or improves test coverage."
    } else {
        roles.push("coder");
        "General task: coder handles implementation."
    };

    (roles, description.into())
}

/// Check if text contains any of the given keywords.
fn contains_any(text: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|kw| text.contains(kw))
}

#[cfg(test)]
#[path = "role_recommend.tests.rs"]
mod tests;
