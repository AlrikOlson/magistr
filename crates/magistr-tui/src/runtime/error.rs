//! What can stop the runtime.

use std::fmt;
use std::io;

use magistr_cockpit::layout::LayoutError;

/// The runtime could not go on.
#[derive(Debug)]
pub enum RuntimeError {
    /// The terminal could not be read, written, sized or taken over.
    Io(io::Error),
    /// The layout engine rejected the view's tree.
    Layout(LayoutError),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "terminal: {e}"),
            Self::Layout(e) => write!(f, "layout: {e}"),
        }
    }
}

impl std::error::Error for RuntimeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Layout(e) => Some(e),
        }
    }
}

impl From<io::Error> for RuntimeError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<LayoutError> for RuntimeError {
    fn from(e: LayoutError) -> Self {
        Self::Layout(e)
    }
}

#[cfg(test)]
#[path = "error.tests.rs"]
mod tests;
