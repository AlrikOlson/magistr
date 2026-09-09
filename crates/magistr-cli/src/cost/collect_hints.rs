//! Collect all applicable optimization hints.

use super::context_bloat_analysis::ContextBloatAnalysis;
use super::optimization::OptimizationHint;
use super::pruning_suggestion::pruning_suggestion;
use super::retry_loop_analysis::RetryLoopAnalysis;
use super::retry_loop_hint::retry_loop_hint;

/// Collect all applicable optimization hints for the current session state.
///
/// Convenience function that runs all detectors and collects non-None hints.
pub fn collect_hints(
    bloat_analysis: Option<&ContextBloatAnalysis>,
    retry_analysis: Option<&RetryLoopAnalysis>,
) -> Vec<OptimizationHint> {
    let mut hints = Vec::new();

    if let Some(bloat) = bloat_analysis
        && let Some(hint) = pruning_suggestion(bloat)
    {
        hints.push(hint);
    }

    if let Some(retry) = retry_analysis
        && let Some(hint) = retry_loop_hint(retry)
    {
        hints.push(hint);
    }

    // Sort by severity (critical first)
    hints.sort_by_key(|h| std::cmp::Reverse(h.severity));
    hints
}
