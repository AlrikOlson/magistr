//! Opaque, clonable session identifier.
//!
//! Uses `Arc<str>` internally so `.clone()` is an O(1) reference count bump
//! rather than a full string copy. Session IDs are cloned frequently during
//! message construction (20+ sites across 7 files).

use std::sync::Arc;

/// Opaque, clonable session identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(Arc<str>);

impl SessionId {
    pub fn new(id: impl Into<String>) -> Self {
        let s: String = id.into();
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
#[path = "session_id.tests.rs"]
mod tests;
