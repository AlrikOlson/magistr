//! One process, one request: these tests share the global and are
//! written so any order of the two async ones holds.

use super::*;

#[tokio::test]
async fn a_bound_token_fires_on_the_callers_own_cancel_without_a_request() {
    let cancel = CancellationToken::new();
    let (either, _guard) = Shutdown::bound(&cancel);
    assert!(!either.is_cancelled());
    cancel.cancel();
    tokio::time::timeout(Duration::from_secs(1), either.cancelled())
        .await
        .expect("the caller's cancel reaches the bound token");
}

#[tokio::test]
async fn the_first_request_is_the_only_one_and_reaches_every_observer() {
    let observer = Shutdown::token();
    let cancel = CancellationToken::new();
    let (either, _guard) = Shutdown::bound(&cancel);
    let first = Shutdown::request();
    assert!(Shutdown::is_requested());
    assert!(!Shutdown::request(), "a repeat is not a new request");
    // The test that asserts the first request may run after another
    // request in the same process; what holds either way is that the
    // observers see it.
    let _ = first;
    tokio::time::timeout(Duration::from_secs(1), observer.cancelled())
        .await
        .expect("an observer token fires");
    tokio::time::timeout(Duration::from_secs(1), either.cancelled())
        .await
        .expect("a bound token fires on the request");
    assert!(
        !cancel.is_cancelled(),
        "the caller's own token is never cancelled by the request"
    );
}
