use serde::Serialize;

/// Inclusive, one-based lines; an empty file has the range zero to zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ReadRange {
    pub start: usize,
    pub end: usize,
}
