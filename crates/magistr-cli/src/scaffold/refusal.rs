use std::fmt;

/// Why an administrative plan refuses to act on a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The target exists but is not a regular file (a directory, a symlink
    /// or a special file), or an ancestor is not a real directory.
    NotRegularFile,
    /// The target exists but could not be read; unreadable is not absent.
    Unreadable {
        /// What went wrong reading it.
        cause: String,
    },
    /// Existing shared configuration cannot be parsed or has the wrong shape.
    InvalidContent { cause: String },
    /// The canonical input this file is rendered from is missing or
    /// unreadable in its home repository, so the embedded copy must not be
    /// written in its place.
    CanonicalUnavailable {
        /// The canonical input, relative to the project root.
        source: String,
        /// What went wrong reading it.
        cause: String,
    },
    /// The file depends on a companion the plan could not prepare.
    MissingDependency {
        /// The companion path, relative to the project root.
        dependency: String,
    },
    /// A template plugin's source file could not be read.
    UnreadableSource {
        /// The source path.
        source: String,
        /// What went wrong reading it.
        cause: String,
    },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidContent { cause } => {
                write!(f, "configuration is invalid; left untouched: {cause}")
            }
            Self::NotRegularFile => f.write_str("target is not a regular file"),
            Self::Unreadable { cause } => write!(f, "target is unreadable: {cause}"),
            Self::CanonicalUnavailable { source, cause } => {
                write!(f, "canonical input {source} is unavailable: {cause}")
            }
            Self::MissingDependency { dependency } => {
                write!(f, "depends on {dependency}, which is not available")
            }
            Self::UnreadableSource { source, cause } => {
                write!(f, "source {source} is unreadable: {cause}")
            }
        }
    }
}
