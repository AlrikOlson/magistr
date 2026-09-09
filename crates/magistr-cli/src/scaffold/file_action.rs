/// What would happen to a file during scaffolding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileAction {
    /// File does not exist — would be created.
    Create,
    /// File exists and content differs — would be overwritten.
    Overwrite,
    /// File exists with identical content — no change needed.
    Unchanged,
    /// File exists and its update policy protects the user's content — skipped.
    Preserved,
}
