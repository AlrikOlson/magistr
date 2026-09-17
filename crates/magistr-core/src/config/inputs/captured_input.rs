use super::InputOrigin;

/// A configuration value together with the origin it was resolved from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedInput<T> {
    pub value: T,
    pub origin: InputOrigin,
}

impl<T> CapturedInput<T> {
    /// A value read from the file at `path`.
    #[must_use]
    pub fn loaded(value: T, path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            value,
            origin: InputOrigin::Loaded(path.into()),
        }
    }

    /// The default value because no file exists at `path`.
    #[must_use]
    pub fn absent(value: T, path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            value,
            origin: InputOrigin::Absent(path.into()),
        }
    }

    /// The default value because no path could be resolved.
    #[must_use]
    pub fn unlocated(value: T) -> Self {
        Self {
            value,
            origin: InputOrigin::Unlocated,
        }
    }
}
