//! What a completion popup completes.

/// The two things the prompt completes: `@` mentions a workspace file,
/// `/` at the start of a line names a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionKind {
    /// `@` — a workspace file.
    File,
    /// `/` at the start of a line — a command.
    Command,
}

impl CompletionKind {
    /// The character that opened the popup.
    #[must_use]
    pub const fn trigger(self) -> char {
        match self {
            Self::File => '@',
            Self::Command => '/',
        }
    }
}
