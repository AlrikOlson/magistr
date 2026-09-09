//! Result of analyzing context for bloat.

use super::bloated_file::BloatedFile;

/// Result of analyzing context for bloat.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextBloatAnalysis {
    /// Files that appear bloated (large + unreferenced).
    pub bloated_files: Vec<BloatedFile>,
    /// Total tokens that could be saved by removing bloated files.
    pub total_reclaimable_tokens: u64,
    /// Estimated cost savings per turn (USD) based on input token pricing.
    pub estimated_savings_per_turn: f64,
}
