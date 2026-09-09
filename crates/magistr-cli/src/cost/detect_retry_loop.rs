//! Detect if the session is in a retry loop.

use super::retry_loop_analysis::RetryLoopAnalysis;
use super::turn_outcome::TurnOutcome;

/// Detect if the session is in a retry loop.
///
/// A retry loop is detected when the same error signature appears
/// in `threshold` or more of the last `window_size` turns.
///
/// `avg_tokens_per_turn` is used to estimate wasted tokens.
pub fn detect_retry_loop(
    outcomes: &[TurnOutcome],
    window_size: usize,
    threshold: u32,
    avg_tokens_per_turn: u64,
) -> RetryLoopAnalysis {
    if outcomes.len() < window_size {
        return RetryLoopAnalysis {
            is_looping: false,
            repeated_error: None,
            consecutive_failures: 0,
            estimated_wasted_tokens: 0,
        };
    }

    let window = &outcomes[outcomes.len().saturating_sub(window_size)..];

    // Count consecutive failures from the end with the same error
    let mut consecutive = 0u32;
    let mut last_error: Option<&str> = None;

    for outcome in window.iter().rev() {
        match outcome {
            TurnOutcome::Failure(err) => {
                if let Some(prev) = last_error {
                    if prev == err {
                        consecutive += 1;
                    } else {
                        break;
                    }
                } else {
                    last_error = Some(err);
                    consecutive = 1;
                }
            }
            TurnOutcome::Success => break,
        }
    }

    let is_looping = consecutive >= threshold;

    RetryLoopAnalysis {
        is_looping,
        repeated_error: if is_looping {
            last_error.map(String::from)
        } else {
            None
        },
        consecutive_failures: consecutive,
        estimated_wasted_tokens: if is_looping {
            // Subtract threshold — the first `threshold` attempts are "reasonable"
            // Only count excess retries as waste
            u64::from(consecutive.saturating_sub(1)) * avg_tokens_per_turn
        } else {
            0
        },
    }
}
