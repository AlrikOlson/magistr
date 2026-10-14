//! The intent fixed when a conversation is created determines its authority.

/// The immutable intent of a conversation and the authority its agent receives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SessionPurpose {
    Conversation,
    Roadmap,
}
