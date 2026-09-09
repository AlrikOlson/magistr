//! Role resolution for a team whose membership comes from a directory.

use crate::error::MagistrError;
use crate::team::{Identity, TeamConfig, TeamRole, resolve_role};

use super::directory_outcome::DirectoryOutcome;

/// Resolve the effective role for `identity`, honouring the directory.
///
/// The rules, in the order they are asked:
///
/// 1. No roster (solo mode) or no `directory` block — this is
///    [`resolve_role`] and the outcome is not consulted at all. A team
///    that has not opted in is unaffected.
/// 2. A roster row that is **deactivated** is a refusal. SCIM's soft
///    delete has to mean something, and the one thing it must not mean
///    is falling through to `default_role`.
/// 3. A roster row that is provisioned answers with its role, and the
///    directory is never asked. The roster *is* the directory's
///    projection — SCIM writes it — so an explicit row is the
///    directory's own answer, already recorded.
/// 4. Off the roster, the outcome decides. [`Unavailable`] is an error:
///    a directory that did not answer grants nothing, and that is
///    checked **before** the CI shortcut, because letting automation
///    past a directory outage is exactly the implicit grant this rule
///    exists to refuse.
/// 5. [`Granted`] is the mapped role; [`NoMatch`] falls through to the
///    behaviour a directory-less team already had — `Ci` under CI,
///    otherwise the roster's `default_role`.
///
/// # Errors
///
/// When the member is deprovisioned, or the directory did not answer.
///
/// [`Unavailable`]: DirectoryOutcome::Unavailable
/// [`Granted`]: DirectoryOutcome::Granted
/// [`NoMatch`]: DirectoryOutcome::NoMatch
pub fn resolve_role_with_directory(
    config: Option<&TeamConfig>,
    identity: &Identity,
    is_ci: bool,
    outcome: &DirectoryOutcome,
) -> Result<TeamRole, MagistrError> {
    let Some(config) = config.filter(|config| !config.directory.is_unset()) else {
        return Ok(resolve_role(config, identity, is_ci));
    };
    if let Some(row) = config.row_for(identity) {
        if !row.active {
            return Err(MagistrError::Permission(format!(
                "'{}' has been deprovisioned by the directory — ask your \
                 administrator to reactivate the account",
                row.id
            )));
        }
        return Ok(row.role);
    }
    match outcome {
        DirectoryOutcome::Unavailable(why) => Err(MagistrError::Permission(format!(
            "the directory could not be reached, so no role can be granted \
             to '{}': {why}",
            identity.id()
        ))),
        DirectoryOutcome::Granted(role) => Ok(*role),
        DirectoryOutcome::NoMatch if is_ci => Ok(TeamRole::Ci),
        DirectoryOutcome::NoMatch => Ok(config.default_role),
    }
}

#[cfg(test)]
#[path = "resolve_role_with_directory.tests.rs"]
mod tests;
