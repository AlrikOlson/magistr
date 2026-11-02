//! What a successful device-grant poll returns.

/// The token endpoint's success response, narrowed to what identity
/// resolution needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantedToken {
    /// The bearer token — used once, to ask the provider who this is,
    /// and never persisted.
    pub access_token: String,
    /// The OIDC identity assertion, when the provider issued one.
    pub id_token: Option<String>,
    /// Seconds until the access token expires, when stated.
    pub expires_in: Option<u64>,
}
