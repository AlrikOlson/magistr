//! Enforce a team's demand for a verified identity.

use crate::error::MagistrError;

use super::super::identity::Identity;
use super::super::team_config::TeamConfig;

/// Refuse an identity the team's `identity_provider` did not vouch for.
///
/// Solo mode and teams that set no provider pass unconditionally — the
/// self-asserted identity stays the default, and turning on
/// verification is an explicit act.
///
/// Signed in with the *wrong* provider is refused as loudly as not
/// being signed in at all: a GitHub login is no evidence at all about
/// an Okta-gated roster.
pub fn require_verified(
    config: Option<&TeamConfig>,
    identity: &Identity,
) -> Result<(), MagistrError> {
    let Some(required) = config.and_then(|config| config.identity_provider.as_ref()) else {
        return Ok(());
    };
    let key = required.key();
    let label = required.label();
    match identity.provider.as_deref() {
        Some(provider) if provider == key => Ok(()),
        Some(other) => Err(MagistrError::Permission(format!(
            "this team requires a verified identity from {label}, but you are signed in \
             with '{other}' — run `magistr login` to sign in with {label}"
        ))),
        None => Err(MagistrError::Permission(format!(
            "this team requires a verified identity from {label} — run `magistr login` \
             (your local git identity is self-asserted and is not accepted here)"
        ))),
    }
}

#[cfg(test)]
#[path = "require_verified.tests.rs"]
mod tests;
