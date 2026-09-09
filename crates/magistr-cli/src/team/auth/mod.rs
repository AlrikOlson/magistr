//! Verified identity — the device-code login that sits above the
//! self-asserted local resolver.
//!
//! [`Identity::resolve`] answers with whatever the machine says it is:
//! `MAGISTR_USER`, git config, the OS user. That is attribution, and it
//! is enough for a solo project. A team that needs *authentication*
//! sets `identity_provider` in `.magistr/team.json`; from then on the
//! identity has to come from a provider that vouched for it.
//!
//! The flow is the OAuth 2.0 device authorization grant (RFC 8628) for
//! both providers: [`IdentityProvider::Github`] against GitHub's built-in
//! endpoints, [`IdentityProvider::Oidc`] against any issuer whose
//! discovery document advertises the grant. [`login`] drives it against
//! the [`DeviceFlow`] port; [`HttpDeviceFlow`] is the one real
//! implementation, and the tests drive a fake instead of the network.
//!
//! A successful login is cached as a [`VerifiedIdentity`] in
//! `~/.magistr/identity.json` ([`IdentityCache`]) with an expiry, and
//! nothing else: magistr does not keep the provider's access token,
//! because it never calls the provider on the user's behalf.
//!
//! [`Identity::resolve`]: super::Identity::resolve

const MAX_ID_TOKEN_BYTES: usize = 32 * 1024;

mod auth_http;
mod cache_path;
mod device_authorization;
mod device_flow;
mod granted_token;
mod http_device_flow;
mod id_token_claims;
mod identity_cache;
mod identity_claims;
mod identity_provider;
mod jwks_cache;
mod login;
mod oidc_metadata;
#[cfg(test)]
mod oidc_test_fixture;
mod poll_schedule;
mod require_verified;
mod secure_endpoint;
mod token_poll;
mod verified_identity;

pub use cache_path::cache_path;
pub use device_authorization::DeviceAuthorization;
pub use device_flow::DeviceFlow;
pub use granted_token::GrantedToken;
pub use http_device_flow::HttpDeviceFlow;
pub use id_token_claims::id_token_claims;
pub use identity_cache::IdentityCache;
pub use identity_claims::IdentityClaims;
pub use identity_provider::IdentityProvider;
pub use login::login;
pub use oidc_metadata::OidcMetadata;
pub use poll_schedule::PollSchedule;
pub use require_verified::require_verified;
pub use token_poll::TokenPoll;
pub use verified_identity::VerifiedIdentity;
