use super::*;

use std::time::Duration;

#[test]
fn default_policy() {
    let policy = ReconnectionPolicy::default();
    assert_eq!(policy.max_retries, 5);
    assert_eq!(policy.initial_backoff, Duration::from_millis(500));
    assert_eq!(policy.max_backoff, Duration::from_secs(30));
    assert_eq!(policy.connect_timeout, Duration::from_secs(10));
    assert_eq!(policy.backoff_multiplier, 2);
}

#[test]
fn no_retry_policy() {
    let policy = ReconnectionPolicy::no_retry();
    assert_eq!(policy.max_retries, 0);
    assert!(!policy.should_retry(0));
}

#[test]
fn builder_methods() {
    let policy = ReconnectionPolicy::default()
        .max_retries(10)
        .initial_backoff(Duration::from_secs(1))
        .max_backoff(Duration::from_secs(60))
        .connect_timeout(Duration::from_secs(5));

    assert_eq!(policy.max_retries, 10);
    assert_eq!(policy.initial_backoff, Duration::from_secs(1));
    assert_eq!(policy.max_backoff, Duration::from_secs(60));
    assert_eq!(policy.connect_timeout, Duration::from_secs(5));
}

#[test]
fn exponential_backoff() {
    let policy = ReconnectionPolicy::default().initial_backoff(Duration::from_millis(100));

    // attempt 0: 100ms * 2^0 = 100ms
    assert_eq!(policy.backoff_for_attempt(0), Duration::from_millis(100));
    // attempt 1: 100ms * 2^1 = 200ms
    assert_eq!(policy.backoff_for_attempt(1), Duration::from_millis(200));
    // attempt 2: 100ms * 2^2 = 400ms
    assert_eq!(policy.backoff_for_attempt(2), Duration::from_millis(400));
    // attempt 3: 100ms * 2^3 = 800ms
    assert_eq!(policy.backoff_for_attempt(3), Duration::from_millis(800));
}

#[test]
fn backoff_capped_at_max() {
    let policy = ReconnectionPolicy::default()
        .initial_backoff(Duration::from_secs(1))
        .max_backoff(Duration::from_secs(5));

    // attempt 3: 1s * 2^3 = 8s, capped at 5s
    assert_eq!(policy.backoff_for_attempt(3), Duration::from_secs(5));
}

#[test]
fn should_retry_within_limit() {
    let policy = ReconnectionPolicy::default().max_retries(3);
    assert!(policy.should_retry(0));
    assert!(policy.should_retry(1));
    assert!(policy.should_retry(2));
    assert!(!policy.should_retry(3));
    assert!(!policy.should_retry(4));
}
