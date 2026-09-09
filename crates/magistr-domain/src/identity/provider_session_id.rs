use alloc::string::String;
use serde::{Deserialize, Serialize};

use super::{IdentityError, ProviderSessionBinding, SessionId, identity_key::IdentityKey};

/// An opaque provider conversation ID qualified by the adapter's provider name.
///
/// A provider ID cannot become a Magistr identity without an explicit binding
/// to a separately allocated, scoped session.
///
/// ```compile_fail
/// use magistr_domain::identity::{ProviderSessionId, SessionId};
/// let remote = ProviderSessionId::new("codex", "thread-1").unwrap();
/// let local: SessionId = remote.into();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProviderSessionId {
    provider: IdentityKey,
    opaque: IdentityKey,
}

impl ProviderSessionId {
    pub fn new(
        provider: impl Into<String>,
        opaque: impl Into<String>,
    ) -> Result<Self, IdentityError> {
        Ok(Self {
            provider: IdentityKey::new(provider)?,
            opaque: IdentityKey::new(opaque)?,
        })
    }

    pub fn provider(&self) -> &str {
        self.provider.as_str()
    }

    pub fn as_str(&self) -> &str {
        self.opaque.as_str()
    }

    #[must_use]
    pub fn bind_to(self, session: SessionId) -> ProviderSessionBinding {
        ProviderSessionBinding::new(session, self)
    }
}
