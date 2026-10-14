//! Host-owned identity, purpose and name of one durable conversation.
use alloc::string::String;

use super::SessionPurpose;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conversation {
    pub id: String,
    pub purpose: SessionPurpose,
    pub name: Option<String>,
    pub provider: String,
}
