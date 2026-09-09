//! Why the engines could not open.

use std::fmt;
use std::io;
use std::path::PathBuf;

/// The store could not be opened — reported before the session starts.
#[derive(Debug)]
pub enum EngineError {
    /// The project root is not a directory.
    NoProject(PathBuf),
    /// The store's directory could not be created.
    Store {
        /// The directory.
        path: PathBuf,
        /// What the file system said.
        source: io::Error,
    },
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoProject(root) => write!(f, "{} is not a project directory", root.display()),
            Self::Store { path, source } => {
                write!(f, "cannot create the store at {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for EngineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NoProject(_) => None,
            Self::Store { source, .. } => Some(source),
        }
    }
}
