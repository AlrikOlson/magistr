//! An image block the protocol carried.

use alloc::string::String;

/// One ACP image content block, exactly as the wire carried it: the
/// base64 payload, its mime type, and where it came from when the block
/// said.
///
/// The cockpit decodes it in the fold, where the terminal's cell size is
/// known — the same block whether it rode a message chunk
/// ([`AgentEvent::Image`]) or a tool call's content
/// ([`AgentEvent::ToolCall`]), so there is one decode path and one
/// rendering route.
///
/// [`AgentEvent::Image`]: super::AgentEvent::Image
/// [`AgentEvent::ToolCall`]: super::AgentEvent::ToolCall
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageBlock {
    /// The block's `mimeType` (`image/png`, …).
    pub mime_type: String,
    /// The block's base64 payload.
    pub data: String,
    /// Where the image came from, when the block said — used as its
    /// label, never fetched: the bytes are already here.
    pub uri: Option<String>,
}
