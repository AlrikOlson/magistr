//! What a directory answered about somebody's groups.

use crate::error::MagistrError;
use crate::team::TeamRole;

use super::group_role::GroupRole;

/// The result of asking a directory which role somebody holds.
///
/// The three variants exist to keep one distinction the whole
/// fail-closed rule rests on: **an answered "no" is data, an unanswered
/// question is a refusal.** A directory that replies "this person is in
/// no mapped group" has told us something, and the roster's own
/// `default_role` applies exactly as it always did. A directory that
/// does not reply has told us nothing, and nothing is not a licence to
/// grant the fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectoryOutcome {
    /// The directory answered and at least one group mapped to a role.
    Granted(TeamRole),
    /// The directory answered; no group it named maps to a role, or it
    /// knows no such subject.
    NoMatch,
    /// The directory did not answer. Carries why, for the refusal.
    Unavailable(String),
}

impl DirectoryOutcome {
    /// Read a group lookup into an outcome.
    ///
    /// The lookup's own type carries the distinction: `Ok(Some(groups))`
    /// is an answer about a known subject, `Ok(None)` is an answer that
    /// the subject is unknown, and `Err` is the absence of an answer.
    #[must_use]
    pub fn from_lookup(
        lookup: Result<Option<Vec<String>>, MagistrError>,
        mappings: &[GroupRole],
    ) -> Self {
        match lookup {
            Ok(Some(groups)) => GroupRole::highest(mappings, &groups)
                .map_or(DirectoryOutcome::NoMatch, DirectoryOutcome::Granted),
            Ok(None) => DirectoryOutcome::NoMatch,
            Err(e) => DirectoryOutcome::Unavailable(e.to_string()),
        }
    }

    /// The groups a directory named, folded straight into an outcome —
    /// the SAML path, where the assertion *is* the answer and there is
    /// no lookup to fail.
    #[must_use]
    pub fn from_groups(groups: &[String], mappings: &[GroupRole]) -> Self {
        GroupRole::highest(mappings, groups)
            .map_or(DirectoryOutcome::NoMatch, DirectoryOutcome::Granted)
    }
}

#[cfg(test)]
#[path = "directory_outcome.tests.rs"]
mod tests;
