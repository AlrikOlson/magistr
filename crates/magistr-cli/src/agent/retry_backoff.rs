//! Execute an async operation with retry and exponential backoff.

use std::future::Future;

use crate::error::MagistrError;

use super::error_kind::ErrorKind;
use super::retry::RetryConfig;
use super::retry_outcome::RetryOutcome;

/// Execute an async operation with retry and exponential backoff.
///
/// The operation is retried only if it fails with a transient error.
/// Permanent errors cause immediate failure without retrying.
///
/// # Arguments
///
/// * `config` - Retry configuration (delays, max retries)
/// * `operation` - A closure that returns a `Future<Output = Result<T, MagistrError>>`
///
/// # Returns
///
/// A `RetryOutcome` containing the final result and the number of attempts made.
pub async fn retry_with_backoff<T, F, Fut>(
    config: &RetryConfig,
    mut operation: F,
) -> RetryOutcome<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, MagistrError>>,
{
    let mut attempts = 0u32;

    loop {
        attempts += 1;
        match operation().await {
            Ok(value) => {
                return RetryOutcome {
                    result: Ok(value),
                    attempts,
                };
            }
            Err(err) => {
                let retries_left = config
                    .max_retries
                    .saturating_sub(attempts.saturating_sub(1));
                if retries_left == 0 || ErrorKind::classify(&err) == ErrorKind::Permanent {
                    return RetryOutcome {
                        result: Err(err),
                        attempts,
                    };
                }
                // Wait before retrying
                let delay = config.delay_for_attempt(attempts - 1);
                tokio::time::sleep(delay).await;
            }
        }
    }
}
