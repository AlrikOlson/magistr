//! Generate a retry loop warning hint.

use super::hint_severity::HintSeverity;
use super::optimization::OptimizationHint;
use super::retry_loop_analysis::RetryLoopAnalysis;

/// Generate a retry loop warning hint.
///
/// Returns `None` if no retry loop is detected.
pub fn retry_loop_hint(analysis: &RetryLoopAnalysis) -> Option<OptimizationHint> {
    if !analysis.is_looping {
        return None;
    }

    let error_desc = analysis
        .repeated_error
        .as_deref()
        .unwrap_or("unknown error");

    let severity = if analysis.consecutive_failures >= 5 {
        HintSeverity::Critical
    } else {
        HintSeverity::Warning
    };

    Some(OptimizationHint {
        title: "Retry loop detected".to_string(),
        description: format!(
            "Session has failed {} consecutive times with the same error: \"{}\". \
             Consider trying a different approach or starting a new session.",
            analysis.consecutive_failures, error_desc,
        ),
        estimated_savings_per_turn: None,
        severity,
    })
}
