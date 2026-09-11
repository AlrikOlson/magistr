use super::TruncationReason;

/// An immutable, complete account of the rows omitted from a result set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Truncation {
    pub(super) dropped: usize,
    pub(super) reason: TruncationReason,
    pub(super) narrow: Vec<String>,
    pub(super) cursor: String,
}

impl Truncation {
    #[must_use]
    pub const fn dropped(&self) -> usize {
        self.dropped
    }
    #[must_use]
    pub const fn reason(&self) -> TruncationReason {
        self.reason
    }
    #[must_use]
    pub fn narrow(&self) -> &[String] {
        &self.narrow
    }
    #[must_use]
    pub fn cursor(&self) -> &str {
        &self.cursor
    }
}
