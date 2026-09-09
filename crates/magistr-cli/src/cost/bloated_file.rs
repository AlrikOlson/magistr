//! A file in context that may be unnecessarily inflating costs.

/// A file in context that may be unnecessarily inflating costs.
#[derive(Debug, Clone, PartialEq)]
pub struct BloatedFile {
    /// File path.
    pub path: String,
    /// Estimated token count for this file.
    pub estimated_tokens: u64,
    /// Number of turns since this file was last referenced.
    pub turns_since_referenced: u32,
}
