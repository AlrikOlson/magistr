//! The device-grant transport port.

use crate::error::MagistrError;

use super::device_authorization::DeviceAuthorization;
use super::granted_token::GrantedToken;
use super::token_poll::TokenPoll;
use super::verified_identity::VerifiedIdentity;

/// How long a cached attestation lasts when the provider states no
/// lifetime of its own. GitHub OAuth App tokens, for instance, never
/// expire — but an attestation magistr will not re-check should.
pub(crate) const DEFAULT_SESSION_SECS: i64 = 12 * 60 * 60;

/// One provider's side of the device authorization grant.
///
/// Three calls, in order: ask for a code, poll until the human answers,
/// then turn the token into an identity. Splitting it this way is what
/// keeps [`login`] free of the network — the tests drive a fake.
///
/// [`login`]: super::login
pub trait DeviceFlow {
    /// Ask the provider for a device code and a user code.
    fn start(&self) -> Result<DeviceAuthorization, MagistrError>;

    /// Poll the token endpoint once.
    fn poll(&self, device_code: &str) -> Result<TokenPoll, MagistrError>;

    /// Turn a granted token into the identity behind it.
    fn identify(&self, token: &GrantedToken) -> Result<VerifiedIdentity, MagistrError>;
}
