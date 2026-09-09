use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use crate::agent::error_kind::ErrorKind;
use crate::agent::retry_backoff::retry_with_backoff;
use crate::error::MagistrError;

// ── RetryConfig tests ──────────────────────────────────────────

#[test]
fn default_config_values() {
    let config = RetryConfig::default();
    assert_eq!(config.max_retries, 3);
    assert_eq!(config.initial_delay, Duration::from_millis(500));
    assert_eq!(config.max_delay, Duration::from_secs(30));
    assert!((config.backoff_factor - 2.0).abs() < f64::EPSILON);
}

#[test]
fn new_sets_max_retries() {
    let config = RetryConfig::new(5);
    assert_eq!(config.max_retries, 5);
    // Other fields remain default
    assert_eq!(config.initial_delay, Duration::from_millis(500));
}

#[test]
fn builder_methods() {
    let config = RetryConfig::new(2)
        .initial_delay(Duration::from_millis(100))
        .max_delay(Duration::from_secs(10))
        .backoff_factor(3.0);

    assert_eq!(config.max_retries, 2);
    assert_eq!(config.initial_delay, Duration::from_millis(100));
    assert_eq!(config.max_delay, Duration::from_secs(10));
    assert!((config.backoff_factor - 3.0).abs() < f64::EPSILON);
}

#[test]
fn delay_for_attempt_exponential() {
    let config = RetryConfig::new(5)
        .initial_delay(Duration::from_millis(100))
        .backoff_factor(2.0)
        .max_delay(Duration::from_secs(60));

    assert_eq!(config.delay_for_attempt(0), Duration::from_millis(100));
    assert_eq!(config.delay_for_attempt(1), Duration::from_millis(200));
    assert_eq!(config.delay_for_attempt(2), Duration::from_millis(400));
    assert_eq!(config.delay_for_attempt(3), Duration::from_millis(800));
}

#[test]
fn delay_for_attempt_caps_at_max() {
    let config = RetryConfig::new(10)
        .initial_delay(Duration::from_millis(1000))
        .backoff_factor(2.0)
        .max_delay(Duration::from_millis(5000));

    // 1000 * 2^0 = 1000
    assert_eq!(config.delay_for_attempt(0), Duration::from_millis(1000));
    // 1000 * 2^1 = 2000
    assert_eq!(config.delay_for_attempt(1), Duration::from_millis(2000));
    // 1000 * 2^2 = 4000
    assert_eq!(config.delay_for_attempt(2), Duration::from_millis(4000));
    // 1000 * 2^3 = 8000 → capped at 5000
    assert_eq!(config.delay_for_attempt(3), Duration::from_millis(5000));
    // Still capped
    assert_eq!(config.delay_for_attempt(10), Duration::from_millis(5000));
}

// ── Error classification tests ─────────────────────────────────

#[test]
fn io_error_is_transient() {
    let err = MagistrError::io("/tmp/foo", std::io::Error::other("oops"));
    assert_eq!(ErrorKind::classify(&err), ErrorKind::Transient);
}

#[test]
fn acp_timeout_is_transient() {
    let err = MagistrError::Acp("request timed out after 30s".to_string());
    assert_eq!(ErrorKind::classify(&err), ErrorKind::Transient);
}

#[test]
fn acp_auth_error_is_permanent() {
    let err = MagistrError::Acp("authentication failed: invalid token".to_string());
    assert_eq!(ErrorKind::classify(&err), ErrorKind::Permanent);
}

#[test]
fn config_error_is_permanent() {
    let err = MagistrError::Config("missing field: name".to_string());
    assert_eq!(ErrorKind::classify(&err), ErrorKind::Permanent);
}

#[test]
fn detection_error_is_permanent() {
    let err = MagistrError::Detection("no language detected".to_string());
    assert_eq!(ErrorKind::classify(&err), ErrorKind::Permanent);
}

#[test]
fn channel_closed_is_transient() {
    let err = MagistrError::Engine("backend channel closed unexpectedly".to_string());
    assert_eq!(ErrorKind::classify(&err), ErrorKind::Transient);
}

#[test]
fn generic_engine_error_is_permanent() {
    let err = MagistrError::Engine("something broke".to_string());
    assert_eq!(ErrorKind::classify(&err), ErrorKind::Permanent);
}

// ── retry_with_backoff tests ───────────────────────────────────

#[tokio::test]
async fn succeeds_on_first_try() {
    let config = RetryConfig::new(3);
    let outcome = retry_with_backoff(&config, || async { Ok::<_, MagistrError>(42) }).await;

    assert!(outcome.result.is_ok());
    assert_eq!(outcome.result.unwrap(), 42);
    assert_eq!(outcome.attempts, 1);
}

#[tokio::test]
async fn retries_on_transient_then_succeeds() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();

    let config = RetryConfig::new(3)
        .initial_delay(Duration::from_millis(1))
        .backoff_factor(1.0);

    let outcome = retry_with_backoff(&config, || {
        let c = counter_clone.clone();
        async move {
            let attempt = c.fetch_add(1, Ordering::SeqCst);
            if attempt < 2 {
                Err(MagistrError::Acp("timeout: request timed out".to_string()))
            } else {
                Ok::<_, MagistrError>("success")
            }
        }
    })
    .await;

    assert!(outcome.result.is_ok());
    assert_eq!(outcome.result.unwrap(), "success");
    assert_eq!(outcome.attempts, 3);
}

#[tokio::test]
async fn fails_immediately_on_permanent_error() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();

    let config = RetryConfig::new(3).initial_delay(Duration::from_millis(1));

    let outcome = retry_with_backoff(&config, || {
        let c = counter_clone.clone();
        async move {
            c.fetch_add(1, Ordering::SeqCst);
            Err::<(), _>(MagistrError::Config("bad config".to_string()))
        }
    })
    .await;

    assert!(outcome.result.is_err());
    // Should only attempt once — permanent error, no retries
    assert_eq!(outcome.attempts, 1);
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn exhausts_retries_on_persistent_transient_error() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();

    let config = RetryConfig::new(2)
        .initial_delay(Duration::from_millis(1))
        .backoff_factor(1.0);

    let outcome = retry_with_backoff(&config, || {
        let c = counter_clone.clone();
        async move {
            c.fetch_add(1, Ordering::SeqCst);
            Err::<(), _>(MagistrError::Acp(
                "503 Service Unavailable — temporarily unavailable".to_string(),
            ))
        }
    })
    .await;

    assert!(outcome.result.is_err());
    // Initial attempt + 2 retries = 3 total
    assert_eq!(outcome.attempts, 3);
    assert_eq!(counter.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn zero_retries_means_single_attempt() {
    let config = RetryConfig::new(0);

    let outcome = retry_with_backoff(&config, || async {
        Err::<(), _>(MagistrError::Acp("connection reset".to_string()))
    })
    .await;

    assert!(outcome.result.is_err());
    assert_eq!(outcome.attempts, 1);
}
