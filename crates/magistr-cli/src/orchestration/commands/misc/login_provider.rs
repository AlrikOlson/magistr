//! Decide which provider a `magistr login` should talk to.

use crate::error::MagistrError;
use crate::team::auth::IdentityProvider;

/// GitHub client ids can come from the environment so a team need not
/// commit one — the id is public, but a fork should not inherit it.
const GITHUB_CLIENT_ID_ENV: &str = "MAGISTR_GITHUB_CLIENT_ID";

/// Resolve the provider from the roster and the command-line flags.
///
/// The roster's `identity_provider` is the default; flags override one
/// field at a time, so `magistr login --client-id X` re-uses the team's
/// issuer. Naming a `kind` that differs from the roster's starts from
/// scratch instead — the user asked for a different provider, not for a
/// patched version of this one.
pub fn login_provider(
    configured: Option<&IdentityProvider>,
    kind: Option<&str>,
    issuer: Option<&str>,
    client_id: Option<&str>,
    scope: Option<&str>,
    env_github_client_id: Option<&str>,
) -> Result<IdentityProvider, MagistrError> {
    let base = match kind {
        Some(kind) => match_kind(kind, configured)?,
        None => configured.cloned().ok_or_else(no_provider)?,
    };
    let scope = scope.map(str::to_string).or_else(|| scope_of(&base));

    match base {
        IdentityProvider::Github { client_id: id, .. } => {
            let client_id = client_id
                .map(str::to_string)
                .or_else(|| non_empty(id))
                .or_else(|| env_github_client_id.map(str::to_string))
                .filter(|id| !id.trim().is_empty())
                .ok_or_else(|| {
                    MagistrError::Other(format!(
                        "GitHub login needs the OAuth app's client id — pass \
                         `--client-id`, set {GITHUB_CLIENT_ID_ENV}, or put it in \
                         .magistr/team.json under identity_provider"
                    ))
                })?;
            Ok(IdentityProvider::Github { client_id, scope })
        }
        IdentityProvider::Oidc {
            issuer: configured_issuer,
            client_id: id,
            ..
        } => {
            let issuer = issuer
                .map(str::to_string)
                .or_else(|| non_empty(configured_issuer))
                .ok_or_else(|| {
                    MagistrError::Other(
                        "OIDC login needs the issuer URL — pass `--issuer`, or put it in \
                         .magistr/team.json under identity_provider"
                            .to_string(),
                    )
                })?;
            let client_id = client_id
                .map(str::to_string)
                .or_else(|| non_empty(id))
                .ok_or_else(|| {
                    MagistrError::Other(
                        "OIDC login needs the client id — pass `--client-id`, or put it in \
                         .magistr/team.json under identity_provider"
                            .to_string(),
                    )
                })?;
            Ok(IdentityProvider::Oidc {
                issuer,
                client_id,
                scope,
            })
        }
    }
}

/// The empty-shell provider for an explicitly named `kind`, seeded from
/// the roster only when it is the same kind.
fn match_kind(
    kind: &str,
    configured: Option<&IdentityProvider>,
) -> Result<IdentityProvider, MagistrError> {
    let same_kind = |provider: &&IdentityProvider| {
        matches!(
            (kind, provider),
            ("github", IdentityProvider::Github { .. }) | ("oidc", IdentityProvider::Oidc { .. })
        )
    };
    if let Some(provider) = configured.filter(same_kind) {
        return Ok(provider.clone());
    }
    match kind {
        "github" => Ok(IdentityProvider::Github {
            client_id: String::new(),
            scope: None,
        }),
        "oidc" => Ok(IdentityProvider::Oidc {
            issuer: String::new(),
            client_id: String::new(),
            scope: None,
        }),
        other => Err(MagistrError::Other(format!(
            "unknown identity provider '{other}' — expected 'github' or 'oidc'"
        ))),
    }
}

/// The provider's own scope, dropping the placeholder default so an
/// override does not get baked in as an explicit choice.
fn scope_of(provider: &IdentityProvider) -> Option<String> {
    match provider {
        IdentityProvider::Github { scope, .. } | IdentityProvider::Oidc { scope, .. } => {
            scope.clone()
        }
    }
}

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

fn no_provider() -> MagistrError {
    MagistrError::Other(
        "no identity provider configured — pass `--provider github --client-id <id>` \
         (or `--provider oidc --issuer <url> --client-id <id>`), or set \
         identity_provider in .magistr/team.json"
            .to_string(),
    )
}

#[cfg(test)]
#[path = "login_provider.tests.rs"]
mod tests;
