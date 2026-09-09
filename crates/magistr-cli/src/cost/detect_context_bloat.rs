//! Detect context bloat from file usage data.

use super::bloated_file::BloatedFile;
use super::context_bloat_analysis::ContextBloatAnalysis;

/// Detect context bloat from file usage data.
///
/// A file is considered "bloated" if:
/// - It has more than `min_tokens` tokens, AND
/// - It hasn't been referenced in more than `stale_turns` turns
///
/// `cost_per_input_token` is used to estimate savings (e.g., $3.00 / 1M tokens = 0.000003).
pub fn detect_context_bloat(
    files: &[BloatedFile],
    min_tokens: u64,
    stale_turns: u32,
    cost_per_input_token: f64,
) -> ContextBloatAnalysis {
    let bloated: Vec<BloatedFile> = files
        .iter()
        .filter(|f| f.estimated_tokens >= min_tokens && f.turns_since_referenced >= stale_turns)
        .cloned()
        .collect();

    let total_reclaimable: u64 = bloated.iter().map(|f| f.estimated_tokens).sum();
    let savings = total_reclaimable as f64 * cost_per_input_token;

    ContextBloatAnalysis {
        bloated_files: bloated,
        total_reclaimable_tokens: total_reclaimable,
        estimated_savings_per_turn: savings,
    }
}
