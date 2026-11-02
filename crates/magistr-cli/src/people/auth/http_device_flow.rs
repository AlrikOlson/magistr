//! The real device flow, over HTTP.

use super::auth_http::AuthHttp;
use super::jwks_cache::JwksCache;
use super::secure_endpoint::secure_endpoint;
use std::cell::{Cell, RefCell};
use std::time::Instant;

use chrono::{DateTime, TimeDelta, Utc};
use serde_json::Value;

use crate::error::MagistrError;

use super::device_authorization::DeviceAuthorization;
use super::device_flow::{DEFAULT_SESSION_SECS, DeviceFlow};
use super::granted_token::GrantedToken;
use super::id_token_claims::id_token_claims;
use super::identity_provider::{GITHUB_DEVICE_CODE_URL, GITHUB_TOKEN_URL, IdentityProvider};
use super::oidc_metadata::OidcMetadata;
use super::token_poll::TokenPoll;
use super::verified_identity::VerifiedIdentity;

/// The device grant's own `grant_type` (RFC 8628 §3.4).
const DEVICE_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";

/// A [`DeviceFlow`] talking to a real provider over HTTPS.
///
/// Endpoints are resolved once, at construction: built in for GitHub,
/// discovered from `/.well-known/openid-configuration` for OIDC. That
/// means an issuer that cannot do the device grant is refused before
/// the human is asked to open a browser.
pub struct HttpDeviceFlow {
    provider: IdentityProvider,
    device_endpoint: String,
    token_endpoint: String,
    metadata: Option<OidcMetadata>,
    http: AuthHttp,
    jwks: RefCell<JwksCache>,
    started_at: Cell<Option<DateTime<Utc>>>,
}

impl HttpDeviceFlow {
    /// Resolve `provider`'s endpoints and get ready to poll.
    pub fn connect(provider: IdentityProvider) -> Result<Self, MagistrError> {
        Self::connect_with_http(provider, AuthHttp::new())
    }

    fn connect_with_http(provider: IdentityProvider, http: AuthHttp) -> Result<Self, MagistrError> {
        if provider.client_id().trim().is_empty() {
            return Err(MagistrError::Other(
                "identity provider requires a nonempty client_id".into(),
            ));
        }
        let metadata = match &provider {
            IdentityProvider::Github { .. } => None,
            IdentityProvider::Oidc { issuer, .. } => {
                secure_endpoint(issuer, true)?;
                if !provider.scope().split(' ').any(|scope| scope == "openid") {
                    return Err(MagistrError::Other(
                        "OIDC device login requires the openid scope".into(),
                    ));
                }
                Some(discover(&http, issuer)?)
            }
        };
        let (device_endpoint, token_endpoint) = metadata.as_ref().map_or_else(
            || {
                (
                    GITHUB_DEVICE_CODE_URL.to_owned(),
                    GITHUB_TOKEN_URL.to_owned(),
                )
            },
            |m| {
                (
                    m.device_authorization_endpoint.clone(),
                    m.token_endpoint.clone(),
                )
            },
        );
        Ok(Self {
            provider,
            device_endpoint,
            token_endpoint,
            metadata,
            http,
            jwks: RefCell::default(),
            started_at: Cell::new(None),
        })
    }

    /// The cache key this flow's logins are stored under.
    #[must_use]
    pub fn provider_key(&self) -> String {
        self.provider.key()
    }

    /// Read successful grants and structured OAuth HTTP 400 errors.
    fn post_form(&self, url: &str, form: &[(&str, &str)]) -> Result<Value, MagistrError> {
        self.http.post_form(url, form)
    }
}

impl DeviceFlow for HttpDeviceFlow {
    fn start(&self) -> Result<DeviceAuthorization, MagistrError> {
        self.started_at.set(None);
        let started_at = Utc::now();
        let value = self.post_form(
            &self.device_endpoint,
            &[
                ("client_id", self.provider.client_id()),
                ("scope", self.provider.scope()),
            ],
        )?;
        if value.get("error").is_some() {
            return Err(MagistrError::Other(
                "identity provider refused the device authorization request".into(),
            ));
        }
        let authorization = DeviceAuthorization::from_json(&value)?;
        secure_endpoint(&authorization.verification_uri, false)?;
        if let Some(uri) = &authorization.verification_uri_complete {
            secure_endpoint(uri, false)?;
        }
        self.started_at.set(Some(started_at));
        Ok(authorization)
    }

    fn poll(&self, device_code: &str) -> Result<TokenPoll, MagistrError> {
        let value = self.post_form(
            &self.token_endpoint,
            &[
                ("client_id", self.provider.client_id()),
                ("device_code", device_code),
                ("grant_type", DEVICE_GRANT_TYPE),
            ],
        )?;
        TokenPoll::from_json(&value)
    }

    fn identify(&self, token: &GrantedToken) -> Result<VerifiedIdentity, MagistrError> {
        match &self.provider {
            IdentityProvider::Oidc {
                issuer, client_id, ..
            } => {
                let raw = token
                    .id_token
                    .as_deref()
                    .filter(|v| !v.is_empty())
                    .ok_or_else(|| {
                        MagistrError::Other(
                            "OIDC token response is missing its required id_token".into(),
                        )
                    })?;
                let started_at = self.started_at.get().ok_or_else(|| {
                    MagistrError::Other("OIDC device authorization has not started".into())
                })?;
                let metadata = self.metadata.as_ref().ok_or_else(|| {
                    MagistrError::Other("OIDC discovery metadata is unavailable".into())
                })?;
                if raw.len() > super::MAX_ID_TOKEN_BYTES {
                    return Err(MagistrError::Other("OIDC id_token exceeds 32 KiB".into()));
                }
                let header = jsonwebtoken::decode_header(raw).map_err(|_| {
                    MagistrError::Other("OIDC id_token has a malformed JWT header".into())
                })?;
                if header.alg != jsonwebtoken::Algorithm::RS256 {
                    return Err(MagistrError::Other("OIDC id_token must use RS256".into()));
                }
                let kid = header
                    .kid
                    .as_deref()
                    .filter(|kid| !kid.trim().is_empty())
                    .ok_or_else(|| {
                        MagistrError::Other("OIDC id_token requires a signing key ID".into())
                    })?;
                let mut cache = self.jwks.borrow_mut();
                let keys = cache.keys_for(kid, Instant::now(), || {
                    let (value, ttl) = self.http.get_json(&metadata.jwks_uri)?;
                    let keys = serde_json::from_value(value).map_err(|_| {
                        MagistrError::Other("OIDC provider returned malformed JWKS".into())
                    })?;
                    Ok((keys, ttl))
                })?;
                let now = Utc::now();
                let claims = id_token_claims(raw, keys, issuer, client_id, now, started_at)?;
                let expires_at = DateTime::from_timestamp(claims.exp, 0).ok_or_else(|| {
                    MagistrError::Other("OIDC id_token expiry is out of range".into())
                })?;
                Ok(VerifiedIdentity {
                    provider: self.provider.key(),
                    subject: claims.sub.clone(),
                    name: claims.display_name().map(str::to_owned),
                    email: claims.email,
                    issued_at: now,
                    expires_at,
                })
            }
            IdentityProvider::Github { .. } => {
                if token.id_token.is_some() {
                    return Err(MagistrError::Other(
                        "GitHub OAuth login does not accept an OIDC id_token".into(),
                    ));
                }
                let user = self.fetch_github_user(&token.access_token)?;
                let now = Utc::now();
                Ok(VerifiedIdentity {
                    provider: self.provider.key(),
                    subject: user.0,
                    name: user.1,
                    email: user.2,
                    issued_at: now,
                    expires_at: expiry(now, None, token.expires_in),
                })
            }
        }
    }
}

impl HttpDeviceFlow {
    /// `(subject, name, email)` for the token's GitHub user.
    fn fetch_github_user(
        &self,
        access_token: &str,
    ) -> Result<(String, Option<String>, Option<String>), MagistrError> {
        let value = self.http.github_user(access_token)?;
        github_identity(&value)
    }
}

/// Fetch and read an issuer's discovery document.
fn discover(http: &AuthHttp, issuer: &str) -> Result<OidcMetadata, MagistrError> {
    secure_endpoint(issuer, true)?;
    let (value, _) = http.get_json(&OidcMetadata::discovery_url(issuer))?;
    let metadata = OidcMetadata::from_json(&value)?;
    metadata.validate_issuer(issuer)?;
    Ok(metadata)
}

fn github_identity(
    value: &Value,
) -> Result<(String, Option<String>, Option<String>), MagistrError> {
    let string = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
    let subject = value
        .get("id")
        .and_then(Value::as_u64)
        .filter(|id| *id > 0)
        .ok_or_else(|| {
            MagistrError::Other("GitHub returned no immutable numeric user ID".into())
        })?;
    Ok((
        subject.to_string(),
        string("name").or_else(|| string("login")),
        string("email"),
    ))
}

/// When the cached attestation stops counting.
///
/// The `id_token`'s own `exp` is the tightest bound and wins; then the
/// token endpoint's `expires_in`; then a fixed session, because an
/// attestation magistr will never re-check must not last forever.
fn expiry(now: DateTime<Utc>, exp: Option<i64>, expires_in: Option<u64>) -> DateTime<Utc> {
    if let Some(exp) = exp {
        return DateTime::from_timestamp(exp, 0).unwrap_or(now);
    }
    let seconds = expires_in
        .unwrap_or(DEFAULT_SESSION_SECS as u64)
        .min(DEFAULT_SESSION_SECS as u64);
    now + TimeDelta::seconds(seconds as i64)
}

#[cfg(test)]
#[path = "http_device_flow.tests.rs"]
mod tests;

#[cfg(test)]
#[path = "http_device_flow.integration.tests.rs"]
mod integration_tests;
