//! The issuer and endpoints that establish the OIDC device-flow trust boundary.

use serde_json::Value;

use crate::error::MagistrError;

/// The device-grant half of an issuer's `openid-configuration`.
///
/// Discovery binds device, token and key endpoints to an exact issuer and
/// requires support for the client's pinned signing algorithm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcMetadata {
    /// The exact issuer identifier; never normalized for comparison.
    pub issuer: String,
    /// Where to ask for a device code.
    pub device_authorization_endpoint: String,
    /// Where to poll for the token.
    pub token_endpoint: String,
    /// Provider-owned public signing keys.
    pub jwks_uri: String,
}

impl OidcMetadata {
    /// The well-known discovery URL for an issuer.
    #[must_use]
    pub fn discovery_url(issuer: &str) -> String {
        format!(
            "{}/.well-known/openid-configuration",
            issuer.trim_end_matches('/')
        )
    }

    /// Read and validate the endpoints and supported signing policy.
    pub fn from_json(value: &Value) -> Result<Self, MagistrError> {
        let string = |key: &str| {
            value
                .get(key)
                .and_then(Value::as_str)
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| {
                    MagistrError::Other(format!(
                        "the issuer's discovery document advertises no {key}"
                    ))
                })
        };
        let metadata = Self {
            device_authorization_endpoint: string("device_authorization_endpoint")?,
            token_endpoint: string("token_endpoint")?,
            issuer: string("issuer")?,
            jwks_uri: string("jwks_uri")?,
        };
        super::secure_endpoint::secure_endpoint(&metadata.issuer, true)?;
        for endpoint in [
            &metadata.device_authorization_endpoint,
            &metadata.token_endpoint,
            &metadata.jwks_uri,
        ] {
            super::secure_endpoint::secure_endpoint(endpoint, false)?;
        }
        let supports_rs256 = value
            .get("id_token_signing_alg_values_supported")
            .and_then(Value::as_array)
            .is_some_and(|algs| {
                algs.iter().all(Value::is_string)
                    && algs.iter().any(|alg| alg.as_str() == Some("RS256"))
            });
        if !supports_rs256 {
            return Err(MagistrError::Other(
                "the issuer must advertise RS256 ID token signatures".into(),
            ));
        }
        Ok(metadata)
    }

    /// Bind discovery to the configured issuer, including its trailing slash.
    pub fn validate_issuer(&self, expected: &str) -> Result<(), MagistrError> {
        if self.issuer != expected {
            return Err(MagistrError::Other(
                "OIDC discovery issuer does not match the configured issuer".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "oidc_metadata.tests.rs"]
mod tests;
