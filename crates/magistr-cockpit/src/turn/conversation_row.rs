//! A source row's measured place in the conversation.
use super::Voice;

/// Source identity and physical bounds stay together for painting, scrolling and picking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConversationRow {
    pub source: usize,
    pub line: usize,
    pub top: usize,
    pub height: usize,
    pub before: usize,
    pub after: usize,
    pub author: Option<Voice>,
    pub card_head: bool,
}
