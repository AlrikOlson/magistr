//! Generate a pruning suggestion hint from a bloat analysis.

use super::context_bloat_analysis::ContextBloatAnalysis;
use super::hint_severity::HintSeverity;
use super::optimization::OptimizationHint;

/// Generate a pruning suggestion hint from a bloat analysis.
///
/// Returns `None` if no files are bloated.
pub fn pruning_suggestion(analysis: &ContextBloatAnalysis) -> Option<OptimizationHint> {
    if analysis.bloated_files.is_empty() {
        return None;
    }

    let file_count = analysis.bloated_files.len();
    let severity = if analysis.estimated_savings_per_turn >= 0.10 {
        HintSeverity::Critical
    } else if analysis.estimated_savings_per_turn >= 0.03 {
        HintSeverity::Warning
    } else {
        HintSeverity::Info
    };

    Some(OptimizationHint {
        title: "Context bloat detected".to_string(),
        description: format!(
            "Removing {} unused file{} from context would save ~${:.2}/turn",
            file_count,
            if file_count == 1 { "" } else { "s" },
            analysis.estimated_savings_per_turn,
        ),
        estimated_savings_per_turn: Some(analysis.estimated_savings_per_turn),
        severity,
    })
}
