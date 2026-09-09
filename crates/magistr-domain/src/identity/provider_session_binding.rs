use serde::{Deserialize, Serialize};

use super::{ProviderSessionId, SessionId};

/// Explicit adapter mapping between a Magistr session and a provider conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSessionBinding {
    session: SessionId,
    provider_session: ProviderSessionId,
}

impl ProviderSessionBinding {
    pub(crate) fn new(session: SessionId, provider_session: ProviderSessionId) -> Self {
        Self {
            session,
            provider_session,
        }
    }

    pub fn session(&self) -> &SessionId {
        &self.session
    }

    pub fn provider_session(&self) -> &ProviderSessionId {
        &self.provider_session
    }
}
