//! The one way layout can fail.

use std::fmt;

use taffy::TaffyError;

/// The layout engine rejected the tree (an index out of range while
/// walking it). Building a tree from a [`super::Node`] cannot produce one,
/// so seeing this means the engine and the node tree disagree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutError {
    message: String,
}

impl LayoutError {
    /// What the engine reported.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "layout failed: {}", self.message)
    }
}

impl std::error::Error for LayoutError {}

impl From<TaffyError> for LayoutError {
    fn from(error: TaffyError) -> Self {
        Self {
            message: error.to_string(),
        }
    }
}

#[cfg(test)]
#[path = "error.tests.rs"]
mod tests;
