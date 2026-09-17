use std::path::{Path, PathBuf};

/// Where a loaded configuration value came from.
///
/// A missing file and an unlocatable path both yield the default value, but
/// they are different facts: one says "the user has not configured this",
/// the other says "this process cannot even name the file". An unreadable
/// or unparseable file is neither; loaders report it as an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputOrigin {
    /// The file at this path was read and parsed.
    Loaded(PathBuf),
    /// No file exists at this path; the default value applies.
    Absent(PathBuf),
    /// No path could be resolved (no config directory); the default applies.
    Unlocated,
}

impl InputOrigin {
    /// The path this origin names, if any.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Loaded(path) | Self::Absent(path) => Some(path),
            Self::Unlocated => None,
        }
    }

    /// Whether a file was actually read.
    #[must_use]
    pub fn is_loaded(&self) -> bool {
        matches!(self, Self::Loaded(_))
    }
}
