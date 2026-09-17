use std::time::Duration;

use super::*;

#[test]
fn a_fresh_link_reads_detached_and_every_clone_sees_the_same_bit() {
    let link = FeedLink::new();
    assert!(!link.is_attached());

    let other = link.clone();
    link.set(true);
    assert!(other.is_attached(), "the clone shares the bit");
    other.set(false);
    assert!(!link.is_attached());
}

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
