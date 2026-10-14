//! What can go wrong between an attached client and its owner.

use std::io;
use std::time::Duration;

use super::attach_refusal::AttachRefusal;
use super::owner_generation::OwnerGeneration;

/// Why an [`OwnerClient`](super::OwnerClient) call did not do what was
/// asked. None of these is a reason to open a writer: an attached surface
/// that cannot reach its owner reports, reconnects, or stands down.
#[derive(Debug)]
pub enum AttachError {
    /// No owner is listening at the endpoint, or it could not be reached.
    Unavailable(io::Error),
    /// The owner answered the handshake with a refusal.
    Refused(AttachRefusal),
    /// The owner hung up: its process or endpoint is gone.
    OwnerGone,
    /// The owner did not answer within the command deadline. The
    /// connection is dropped, since a late answer could no longer be told
    /// from the next; reconnect before the next command.
    Deadline(Duration),
    /// The command carried a generation that is not the owner's: the owner
    /// was replaced since this client attached, and what the client is
    /// looking at may be stale.
    StaleGeneration {
        /// The owner's current generation.
        current: OwnerGeneration,
    },
    /// The owner's writer refused the command, in the engine's own words.
    Engine(Box<super::WorkflowMutationError>),
    /// The other end sent something that is not the protocol.
    Wire(String),
    /// The client is not attached; reconnect first.
    Detached,
    /// This platform has no Unix domain sockets.
    Unsupported,
}

impl std::fmt::Display for AttachError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(error) => write!(f, "no owner endpoint answered: {error}"),
            Self::Refused(refusal) => write!(f, "the owner refused the attachment: {refusal}"),
            Self::OwnerGone => write!(f, "the owner is gone"),
            Self::Deadline(limit) => {
                write!(f, "the owner did not answer within {}ms", limit.as_millis())
            }
            Self::StaleGeneration { current } => write!(
                f,
                "the owner was replaced (now generation {current}); reconnect and re-read"
            ),
            Self::Engine(why) => write!(f, "{why}"),
            Self::Wire(why) => write!(f, "malformed owner protocol: {why}"),
            Self::Detached => write!(f, "not attached to an owner"),
            Self::Unsupported => write!(f, "owner attachment needs Unix domain sockets"),
        }
    }
}

impl std::error::Error for AttachError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(error) => Some(error),
            _ => None,
        }
    }
}
