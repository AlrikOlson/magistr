//! Which identity provider a team trusts, and how to reach it.

use base64::prelude::*;
use serde::{Deserialize, Serialize};

/// GitHub's device-authorization endpoint (RFC 8628 §3.1).
pub(crate) const GITHUB_DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
/// GitHub's token endpoint for the device grant.
pub(crate) const GITHUB_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
/// GitHub's authenticated-user endpoint — the identity behind the token.
pub(crate) const GITHUB_USER_URL: &str = "https://api.github.com/user";

/// Default GitHub scopes: enough to read the signed-in user and their
/// verified email, nothing more.
pub(crate) const GITHUB_DEFAULT_SCOPE: &str = "read:user user:email";
/// Default OIDC scopes: the identity claims and nothing else.
pub(crate) const OIDC_DEFAULT_SCOPE: &str = "openid profile email";

/// A verified-identity provider — the `identity_provider` field of
/// `.magistr/team.json`, or the flags of a one-off `magistr login`.
///
/// Both variants speak the OAuth 2.0 device authorization grant
/// (RFC 8628): the CLI asks for a user code, the human types it into a
/// browser, and the CLI polls until the provider issues a token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum IdentityProvider {
    /// GitHub's device flow. Endpoints are built in; the `client_id` is
    /// the OAuth App or GitHub App the team registered.
    Github {
        /// The registered app's client id.
        client_id: String,
        /// Requested scopes; defaults to `read:user user:email`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scope: Option<String>,
    },
    /// Any OIDC issuer whose discovery document advertises a device
    /// authorization endpoint.
    Oidc {
        /// The issuer URL; discovery appends `/.well-known/openid-configuration`.
        issuer: String,
        /// The registered client id.
        client_id: String,
        /// Requested scopes; defaults to `openid profile email`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scope: Option<String>,
    },
}

impl IdentityProvider {
    /// The stable cache key: `github`, or the exact OIDC issuer and client ID.
    /// A token verified for one client must not authorize another client's team.
    #[must_use]
    pub fn key(&self) -> String {
        match self {
            IdentityProvider::Github { .. } => "github".to_string(),
            IdentityProvider::Oidc {
                issuer, client_id, ..
            } => {
                let client = BASE64_URL_SAFE_NO_PAD.encode(client_id.as_bytes());
                format!("oidc:{issuer}#client_id={client}")
            }
        }
    }

    /// A one-line description for CLI output.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            IdentityProvider::Github { .. } => "GitHub".to_string(),
            IdentityProvider::Oidc { issuer, .. } => format!("OIDC ({issuer})"),
        }
    }

    /// The registered client id.
    #[must_use]
    pub fn client_id(&self) -> &str {
        match self {
            IdentityProvider::Github { client_id, .. }
            | IdentityProvider::Oidc { client_id, .. } => client_id,
        }
    }

    /// The scopes to request, provider default when unset.
    #[must_use]
    pub fn scope(&self) -> &str {
        match self {
            IdentityProvider::Github { scope, .. } => {
                scope.as_deref().unwrap_or(GITHUB_DEFAULT_SCOPE)
            }
            IdentityProvider::Oidc { scope, .. } => scope.as_deref().unwrap_or(OIDC_DEFAULT_SCOPE),
        }
    }
}

#[cfg(test)]
#[path = "identity_provider.tests.rs"]
mod tests;
