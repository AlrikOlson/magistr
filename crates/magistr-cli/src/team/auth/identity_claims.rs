//! The identity claims magistr reads out of an OIDC `id_token`.

use serde::Deserialize;

/// The subset of OIDC standard claims that becomes an [`Identity`].
///
/// Unknown claims are ignored — an issuer is free to send fifty of them.
///
/// [`Identity`]: crate::team::Identity
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct IdentityClaims {
    /// The issuer-local stable subject id.
    pub sub: String,
    /// The end user's email, when the `email` scope was granted.
    #[serde(default)]
    pub email: Option<String>,
    /// The end user's display name.
    #[serde(default)]
    pub name: Option<String>,
    /// The shorthand login name, when the issuer sends one.
    #[serde(default)]
    pub preferred_username: Option<String>,
    /// The token's own expiry, as a Unix timestamp.
    pub exp: i64,
    /// When the issuer created the token, as a Unix timestamp.
    pub iat: i64,
}

impl IdentityClaims {
    /// The best display name in the claims: `name`, then
    /// `preferred_username`. Blank strings do not count.
    #[must_use]
    pub fn display_name(&self) -> Option<&str> {
        fn non_blank(value: Option<&str>) -> Option<&str> {
            value.map(str::trim).filter(|v| !v.is_empty())
        }
        non_blank(self.name.as_deref()).or_else(|| non_blank(self.preferred_username.as_deref()))
    }
}
