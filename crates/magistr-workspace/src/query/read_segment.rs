use serde::Serialize;

/// A contiguous span of complete source lines, without a trailing newline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReadSegment {
    pub file: String,
    pub start: usize,
    pub end: usize,
    pub text: String,
}
