use std::time::Duration;

use super::*;

#[tokio::test]
async fn a_request_made_before_anyone_waits_is_kept_and_several_collapse_into_one() {
    let link = FeedLink::new();
    let waiter = link.clone();

    link.request_snapshot();
    link.request_snapshot();
    // The permit is there: the wait resolves at once.
    tokio::time::timeout(Duration::from_secs(1), waiter.snapshot_requested())
        .await
        .expect("a kept request resolves the wait");
    // Two requests were one permit: the next wait blocks until asked again.
    assert!(
        tokio::time::timeout(Duration::from_millis(50), waiter.snapshot_requested())
            .await
            .is_err(),
        "no second permit"
    );
    link.request_snapshot();
    tokio::time::timeout(Duration::from_secs(1), waiter.snapshot_requested())
        .await
        .expect("a new request resolves the next wait");
}
