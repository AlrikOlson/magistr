//! The process-wide request is exercised in a child so it cannot cancel sibling tests.

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
    const CHILD: &str = "MAGISTR_TEST_SHUTDOWN_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "utils::shutdown::tests::the_first_request_is_the_only_one_and_reaches_every_observer", "--nocapture"])
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert!(status.success(), "shutdown child failed: {status}");
        return;
    }
    let observer = Shutdown::token();
    let cancel = CancellationToken::new();
    let (either, _guard) = Shutdown::bound(&cancel);
    let first = Shutdown::request();
    assert!(Shutdown::is_requested());
    assert!(!Shutdown::request(), "a repeat is not a new request");
    assert!(first, "the first request belongs to this fresh process");
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
