//! Which of F1's three ways in is being typed.

/// The field open on the ground card (Birth F1) while a path or a URL
/// is being typed: the slot's `click to browse` takes a folder path,
/// `Clone a repository…` a URL, `Start empty` the folder to create.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GroundField {
    /// A folder's path.
    Browse,
    /// A repository's URL.
    Clone,
    /// A new folder's path.
    Empty,
}

impl GroundField {
    /// What the field asks for.
    #[must_use]
    pub const fn placeholder(self) -> &'static str {
        match self {
            Self::Browse => "Path to a folder",
            Self::Clone => "Repository URL",
            Self::Empty => "Folder to create",
        }
    }
}
