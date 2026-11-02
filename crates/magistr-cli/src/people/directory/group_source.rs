//! The port a directory answers group membership through.

use crate::error::MagistrError;

/// Something that can say which groups a subject belongs to.
///
/// The return type is the whole contract, because the fail-closed rule
/// is decided by it and not by the implementation:
///
/// - `Ok(Some(groups))` — the directory answered about a subject it
///   knows.
/// - `Ok(None)` — the directory answered, and knows no such subject.
///   That is an answer, and it resolves to
///   [`DirectoryOutcome::NoMatch`].
/// - `Err(_)` — the directory did **not** answer: unreachable, refused
///   the bind, timed out. That resolves to
///   [`DirectoryOutcome::Unavailable`] and refuses the session.
///
/// An implementation that reports a connection failure as `Ok(None)` is
/// therefore turning a refusal into a fallback, which is the one bug
/// this trait's shape exists to make hard to write.
///
/// [`DirectoryOutcome::NoMatch`]: super::DirectoryOutcome::NoMatch
/// [`DirectoryOutcome::Unavailable`]: super::DirectoryOutcome::Unavailable
pub trait GroupSource {
    /// The groups `subject` belongs to.
    ///
    /// # Errors
    ///
    /// When the directory could not be asked. The caller fails closed.
    fn groups(&self, subject: &str) -> Result<Option<Vec<String>>, MagistrError>;
}
