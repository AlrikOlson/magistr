//! Lossless text and encoded structured data carried by tool observations.

use alloc::string::String;
use core::fmt;

/// The raw data a tool supplied, without a JSON library in the application.
/// Hosts encode non-string JSON values compactly; strings keep their own text.
/// Truncation turns encoded JSON into text so a prefix never claims to be JSON.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolPayload {
    /// A string value, or the retained prefix of an oversized value.
    Text(String),
    /// A complete compact JSON value encoded by the host.
    Json(String),
}

impl ToolPayload {
    /// The bytes displayed and counted toward the record's storage limit.
    #[must_use]
    pub fn content(&self) -> &str {
        match self {
            Self::Text(text) | Self::Json(text) => text,
        }
    }

    /// The string value, if this is text rather than structured JSON.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Json(_) => None,
        }
    }
}

impl fmt::Display for ToolPayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.content())
    }
}
