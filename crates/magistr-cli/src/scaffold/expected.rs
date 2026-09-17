/// What a plan expects to find at a path when it is applied: nothing, or
/// exactly the bytes that were read while planning.
///
/// The hash is the scaffold manifest's content hash, so an application step
/// can compare it with the manifest record as well as with the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expected {
    /// No file at the path.
    Absent,
    /// A file whose content hashes to `hash`.
    Content {
        /// Hex-encoded content hash (see `manifest::content_hash`).
        hash: String,
    },
}

impl Expected {
    /// The expectation for content read while planning.
    #[must_use]
    pub fn of(existing: Option<&str>) -> Self {
        match existing {
            None => Self::Absent,
            Some(content) => Self::Content {
                hash: super::manifest::content_hash(content),
            },
        }
    }

    /// The lower-case description machine-readable output uses.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Absent => "absent".to_owned(),
            Self::Content { hash } => hash.clone(),
        }
    }
}
