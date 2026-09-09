//! A cached identity that a provider vouched for.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// An identity attested by a provider, with the instant its attestation
/// stops counting.
///
/// This is the whole of what magistr keeps: the subject, the display
/// fields and the two timestamps. The provider's access token is
/// deliberately *not* stored — magistr never calls the provider on the
/// user's behalf, so keeping the bearer token would be a liability with
/// no use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifiedIdentity {
    /// The [`super::IdentityProvider::key`] that issued this identity.
    pub provider: String,
    /// The provider's stable subject id (`sub`, or the GitHub user id).
    pub subject: String,
    /// Display name when the provider gave one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Email when the provider gave one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// When the login happened — the tiebreak when several providers
    /// are cached and none is preferred.
    pub issued_at: DateTime<Utc>,
    /// When the attestation stops being trusted.
    pub expires_at: DateTime<Utc>,
}

impl VerifiedIdentity {
    /// Has the attestation aged out as of `now`?
    #[must_use]
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        now >= self.expires_at
    }

    /// The best display name available: the provider's `name`, else the
    /// email local part, else the subject.
    #[must_use]
    pub fn display_name(&self) -> &str {
        if let Some(name) = self.name.as_deref().filter(|n| !n.trim().is_empty()) {
            return name;
        }
        if let Some(email) = self.email.as_deref().filter(|e| !e.trim().is_empty()) {
            return email.split('@').next().unwrap_or(email);
        }
        &self.subject
    }
}

#[cfg(test)]
#[path = "verified_identity.tests.rs"]
mod tests;
