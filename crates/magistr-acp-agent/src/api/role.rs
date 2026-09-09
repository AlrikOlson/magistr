//! Who said a message.

/// The two sides of a chat history. Each API spells them its own way
/// (`assistant` / `model`); the [`ChatApi`](crate::ChatApi) translates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The client's prompt.
    User,
    /// The model's reply.
    Assistant,
}
