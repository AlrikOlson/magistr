use alloc::string::String;
use magistr_domain::identity::{ProviderSessionBinding, SessionId, TurnId};

/// An intent addressed to independently allocated Magistr session identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionCommand {
    /// Open a fresh conversation after resolving the provider and model choice.
    /// `None` explicitly permits the resolved provider's default model.
    Start {
        session: SessionId,
        provider: String,
        model: Option<String>,
    },
    /// Restore a previously bound conversation. The binding does not prove that
    /// restoration is supported; the adapter must verify that capability.
    Resume { binding: ProviderSessionBinding },
    /// Submit a prompt for a fresh turn within an existing session.
    Prompt {
        turn: TurnId,
        content: super::PromptInput,
    },
    /// Request cancellation of this turn, without closing its session.
    CancelTurn { turn: TurnId },
    /// Terminate the live session and release its resources.
    Close { session: SessionId },
}

impl SessionCommand {
    /// The session whose state this command addresses.
    #[must_use]
    pub fn session(&self) -> &SessionId {
        match self {
            Self::Start { session, .. } | Self::Close { session } => session,
            Self::Resume { binding } => binding.session(),
            Self::Prompt { turn, .. } | Self::CancelTurn { turn } => turn.session(),
        }
    }
}
