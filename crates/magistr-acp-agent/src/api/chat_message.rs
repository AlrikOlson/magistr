//! One message of a session's history.

use crate::Role;

/// One turn of the conversation as the provider keeps it — the history
/// every request carries in full, since none of these APIs holds a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    /// Who said it.
    pub role: Role,
    /// What was said.
    pub content: String,
}

impl ChatMessage {
    /// A message from `role`.
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }
}
