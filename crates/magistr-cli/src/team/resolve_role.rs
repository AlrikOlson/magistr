//! Map an identity to its effective role.

use super::identity::Identity;
use super::team_config::TeamConfig;
use super::team_role::TeamRole;

/// Resolve the effective role for `identity`.
///
/// No config (solo mode) → `Owner`. On the roster → the member's role.
/// Off the roster: `Ci` when running under a CI environment, otherwise
/// the config's `default_role`.
#[must_use]
pub fn resolve_role(config: Option<&TeamConfig>, identity: &Identity, is_ci: bool) -> TeamRole {
    let Some(config) = config else {
        return TeamRole::Owner;
    };
    match config.member_for(identity) {
        Some(member) => member.role,
        None if is_ci => TeamRole::Ci,
        None => config.default_role,
    }
}

#[cfg(test)]
#[path = "resolve_role.tests.rs"]
mod tests;
