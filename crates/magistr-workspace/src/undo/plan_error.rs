/// An invalid planning input; no write or journal effect has occurred.
#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum UndoPlanError {
    #[error("undo requires an admitted snapshot for {0}")]
    MissingSnapshot(String),
    #[error(
        "undo candidates require at most 100 distinct non-undo operations in descending sequence order and distinct paths within each operation"
    )]
    InvalidCandidates,
}
