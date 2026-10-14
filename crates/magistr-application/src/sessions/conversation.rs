//! Host-owned identity and name of one durable conversation.
use alloc::string::String;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conversation {
    pub id: String,
    pub name: Option<String>,
    pub provider: String,
}
