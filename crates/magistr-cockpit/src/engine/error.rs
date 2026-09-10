//! Why the engines could not open.

use std::fmt;
use std::io;
use std::path::PathBuf;

use magistr_engine::AttachError;

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
    /// The project host lease was refused: another cockpit or loop already
    /// holds this project on this store, or its lock could not be taken.
    Admission {
        /// The project root.
        root: PathBuf,
        /// The lease's own refusal.
        source: io::Error,
    },
    /// Another cockpit or loop holds the project and its owner endpoint
    /// did not take this process: nothing here may open a second writer,
    /// so the cockpit fails closed. The observer cockpit (bare `magistr`)
    /// is the explicit read-only way in.
    Occupied {
        /// The project root.
        root: PathBuf,
        /// The lease's refusal.
        lease: io::Error,
        /// Why attaching to the running host failed.
        attach: AttachError,
    },
    /// This host could not serve its owner endpoint.
    Endpoint {
        /// The socket.
        path: PathBuf,
        /// What the endpoint said.
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
            Self::Admission { root, source } => {
                write!(f, "{} is not admitted as a host: {source}", root.display())
            }
            Self::Occupied {
                root,
                lease,
                attach,
            } => write!(
                f,
                "{} is held by another cockpit or loop ({lease}) and its owner endpoint did not \
                 take this cockpit ({attach}); close the other host, or run the observer cockpit \
                 (`magistr`) to watch read-only",
                root.display()
            ),
            Self::Endpoint { path, source } => write!(
                f,
                "cannot serve the owner endpoint at {}: {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for EngineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NoProject(_) => None,
            Self::Store { source, .. }
            | Self::Admission { source, .. }
            | Self::Endpoint { source, .. } => Some(source),
            Self::Occupied { attach, .. } => Some(attach),
        }
    }
}
