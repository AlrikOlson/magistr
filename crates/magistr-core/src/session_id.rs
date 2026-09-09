//! Legacy provider-facing session identifier used by agent messages.
//!
//! This unscoped wire value is distinct from `magistr_domain::identity::SessionId`.
//! Use `bind_provider_id` to explicitly associate it with a Magistr session.
//!
//! Uses `Arc<str>` internally so `.clone()` is an O(1) reference count bump
//! rather than a full string copy. Session IDs are cloned frequently during
//! message construction (20+ sites across 7 files).

use std::sync::Arc;

/// Legacy opaque provider session identifier; see `bind_provider_id` for domain correlation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(Arc<str>);

impl SessionId {
    pub fn new(id: impl Into<String>) -> Self {
        let s: String = id.into();
        Self(s.into())
    }

    /// Bind this provider wire ID to an independently allocated Magistr session.
    pub fn bind_provider_id(
        &self,
        provider: impl Into<String>,
        session: magistr_domain::identity::SessionId,
    ) -> Result<
        magistr_domain::identity::ProviderSessionBinding,
        magistr_domain::identity::IdentityError,
    > {
        magistr_domain::identity::ProviderSessionId::new(provider, self.as_str())
            .map(|provider_id| provider_id.bind_to(session))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
#[path = "session_id.tests.rs"]
mod tests;
