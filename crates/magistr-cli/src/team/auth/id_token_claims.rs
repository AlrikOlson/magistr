//! Authenticate an OIDC ID token with the discovered issuer's signing keys.

use chrono::{DateTime, Utc};
use jsonwebtoken::jwk::{
    AlgorithmParameters, Jwk, JwkSet, KeyAlgorithm, KeyOperations, PublicKeyUse,
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::{Deserialize, Deserializer};

use crate::error::MagistrError;

use super::MAX_ID_TOKEN_BYTES;
use super::identity_claims::IdentityClaims;

const CLOCK_SKEW_SECONDS: i64 = 60;

/// Verify an ID token before returning identity claims.
///
/// Only RS256 is accepted, using one unambiguous `kid` in the issuer's
/// discovered JWKS. Header key URLs and embedded keys never supply trust.
/// `now` and `started_at` bind validity to this device authorization attempt;
/// the one-minute issue-time skew does not extend the token's expiry.
/// RFC 8628 defines no nonce parameter, so this flow rejects unexpected nonces.
pub fn id_token_claims(
    token: &str,
    jwks: &JwkSet,
    issuer: &str,
    client_id: &str,
    now: DateTime<Utc>,
    started_at: DateTime<Utc>,
) -> Result<IdentityClaims, MagistrError> {
    if token.len() > MAX_ID_TOKEN_BYTES || issuer.is_empty() || client_id.is_empty() {
        return Err(invalid("invalid token or verification context"));
    }
    let header = decode_header(token).map_err(|_| invalid("malformed JWT header"))?;
    if header.alg != Algorithm::RS256 {
        return Err(invalid("only RS256 signatures are supported"));
    }
    if header.crit.is_some()
        || header.enc.is_some()
        || header.zip.is_some()
        || header.extras.inner().contains_key("b64")
    {
        return Err(invalid("unsupported JOSE header extension"));
    }
    let kid = header
        .kid
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| invalid("missing signing key identifier"))?;
    let mut matching = jwks
        .keys
        .iter()
        .filter(|key| key.common.key_id.as_deref() == Some(kid));
    let jwk = matching
        .next()
        .ok_or_else(|| invalid("unknown signing key identifier"))?;
    if matching.next().is_some() {
        return Err(invalid("ambiguous signing key identifier"));
    }
    let key = signing_key(jwk)?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_required_spec_claims(&["iss", "sub", "aud", "exp"]);
    validation.set_issuer(&[issuer]);
    validation.set_audience(&[client_id]);
    validation.leeway = 0;
    // The library uses the wall clock. Validate time below against the caller's
    // single clock sample so authentication and cache expiry share a boundary.
    validation.validate_exp = false;
    validation.validate_nbf = false;
    let claims = decode::<TokenClaims>(token, &key, &validation)
        .map_err(|_| invalid("signature or required claims are invalid"))?
        .claims;
    claims.validate(issuer, client_id, now, started_at)?;
    Ok(claims.identity)
}

fn signing_key(jwk: &Jwk) -> Result<DecodingKey, MagistrError> {
    let common = &jwk.common;
    if !matches!(jwk.algorithm, AlgorithmParameters::RSA(_))
        || common
            .key_algorithm
            .as_ref()
            .is_some_and(|algorithm| *algorithm != KeyAlgorithm::RS256)
        || common
            .public_key_use
            .as_ref()
            .is_some_and(|usage| *usage != PublicKeyUse::Signature)
        || common
            .key_operations
            .as_ref()
            .is_some_and(|operations| operations.as_slice() != [KeyOperations::Verify])
    {
        return Err(invalid(
            "signing key is incompatible with RS256 verification",
        ));
    }
    DecodingKey::from_jwk(jwk).map_err(|_| invalid("malformed signing key"))
}

#[derive(Deserialize)]
struct TokenClaims {
    #[serde(flatten)]
    identity: IdentityClaims,
    iss: String,
    aud: Audience,
    #[serde(default, deserialize_with = "present")]
    azp: Option<String>,
    #[serde(default, deserialize_with = "present")]
    nbf: Option<i64>,
    #[serde(default, deserialize_with = "present")]
    nonce: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Audience {
    One(String),
    Many(Vec<String>),
}

impl TokenClaims {
    fn validate(
        &self,
        issuer: &str,
        client_id: &str,
        now: DateTime<Utc>,
        started_at: DateTime<Utc>,
    ) -> Result<(), MagistrError> {
        let trusted_audience = match &self.aud {
            Audience::One(value) => value == client_id,
            Audience::Many(values) => {
                !values.is_empty() && values.iter().all(|value| value == client_id)
            }
        };
        if self.iss != issuer
            || !trusted_audience
            || self.azp.as_deref().is_some_and(|value| value != client_id)
        {
            return Err(invalid(
                "issuer, audience or authorized party does not match",
            ));
        }
        let identity = &self.identity;
        if identity.sub.trim().is_empty() || !identity.sub.is_ascii() || identity.sub.len() > 255 {
            return Err(invalid("missing or invalid stable subject"));
        }
        let now = now.timestamp();
        let started_at = started_at.timestamp();
        if started_at > now
            || identity.exp <= now
            || identity.iat < started_at.saturating_sub(CLOCK_SKEW_SECONDS)
            || identity.iat > now.saturating_add(CLOCK_SKEW_SECONDS)
            || identity.iat >= identity.exp
            || self.nbf.is_some_and(|nbf| nbf > now)
        {
            return Err(invalid("expired token or invalid issue/validity time"));
        }
        if self.nonce.is_some() {
            return Err(invalid("unexpected nonce for device authorization"));
        }
        Ok(())
    }
}

// Missing optional claims are allowed; a present null or wrong type is not.
fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

fn invalid(reason: &str) -> MagistrError {
    MagistrError::Permission(format!("OIDC ID token validation failed: {reason}"))
}

#[cfg(test)]
#[path = "id_token_claims.tests.rs"]
mod tests;
