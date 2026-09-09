//! One group→role row of a team's directory mapping.

use serde::{Deserialize, Serialize};

use magistr_engine::policy::glob_match;

use crate::team::TeamRole;

/// A directory group, and the role membership of it buys.
///
/// `group` is a shell-style glob (the same anchored `*`/`?` matcher the
/// governance policy uses, so a team learns one pattern language), which
/// is what lets one row cover `eng-*` rather than needing a row per
/// squad.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupRole {
    /// The directory group name, or a glob over group names.
    pub group: String,
    /// The role a member of that group holds.
    pub role: TeamRole,
}

impl GroupRole {
    /// Whether this row covers `group`.
    #[must_use]
    pub fn matches(&self, group: &str) -> bool {
        glob_match(&self.group, group)
    }

    /// The role `groups` earns under `mappings`, or `None` when nothing
    /// matched.
    ///
    /// Several rows may match at once — a person is in more than one
    /// group, and the globs overlap — so the answer is the **most
    /// privileged** match. That is the same trick
    /// [`PolicyEngine::evaluate`] plays with severity: folding an
    /// extremum over every match means the order rows are written in
    /// cannot change the answer.
    ///
    /// [`PolicyEngine::evaluate`]: magistr_engine::policy::PolicyEngine::evaluate
    #[must_use]
    pub fn highest(mappings: &[GroupRole], groups: &[String]) -> Option<TeamRole> {
        groups
            .iter()
            .flat_map(|group| mappings.iter().filter(move |row| row.matches(group)))
            .map(|row| row.role)
            .max_by_key(|role| role.privilege())
    }
}

#[cfg(test)]
#[path = "group_role.tests.rs"]
mod tests;
