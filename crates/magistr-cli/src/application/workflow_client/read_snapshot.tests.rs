use super::*;
use magistr_cockpit::harness::fixtures;
use magistr_engine::{NativeConfig, ReadState};
use std::time::Duration;

fn fixture() -> (tempfile::TempDir, SnapshotReads) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let config = NativeConfig::from_lookup(root, |_| None).with_data_dir(temp.path().join("data"));
    let reader = NativeReader::open_for_project(&config, "read-order".into());
    (temp, SnapshotReads::new(reader))
}

#[tokio::test]
async fn failed_worker_retains_success_and_marks_unread_families_unavailable() {
    let before = fixtures::snapshot(vec![fixtures::chunk("selected")], Vec::new());
    let failed = read_with(before.clone(), || panic!("injected worker failure")).await;
    assert_eq!(
        failed.roadmap.last_successful(),
        before.roadmap.last_successful()
    );
    assert!(matches!(
        failed.roadmap,
        ReadState::Stale {
            error: ReadError {
                kind: ReadErrorKind::Interrupted,
                ..
            },
            ..
        }
    ));
    assert!(matches!(failed.ship, ReadState::Unavailable(_)));
    assert!(failed.needs_retry());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delayed_read_publishes_before_a_later_failure_and_cannot_resurrect_current() {
    let (_temp, reads) = fixture();
    let (inbox, mut messages) = Inbox::channel();
    let initial = fixtures::snapshot(vec![fixtures::chunk("selected")], Vec::new());
    let (started, waiting) = tokio::sync::oneshot::channel();
    let (release, held) = std::sync::mpsc::channel();
    let first_reads = reads.clone();
    let first_inbox = inbox.clone();
    let first = tokio::spawn(async move {
        first_reads
            .observe(Some(&first_inbox), true, move || {
                started.send(()).unwrap();
                held.recv().unwrap();
                initial
            })
            .await
    });
    waiting.await.unwrap();
    let later_reads = reads.clone();
    let second = tokio::spawn(async move {
        later_reads
            .observe(Some(&inbox), true, || panic!("later read failed"))
            .await
    });
    assert!(messages.try_recv().is_err());
    release.send(()).unwrap();
    first.await.unwrap();
    second.await.unwrap();

    let Some(Msg::Engine(EngineEvent::Seed(first))) = messages.recv().await else {
        panic!("first read");
    };
    let Some(Msg::Engine(EngineEvent::Seed(second))) = messages.recv().await else {
        panic!("second read");
    };
    assert!(first.roadmap.current().is_ok());
    assert!(second.roadmap.current().is_err());
    assert_eq!(
        second.roadmap.last_successful(),
        first.roadmap.last_successful()
    );
    assert_eq!(*reads.seen.lock().await, second);
}

#[tokio::test]
async fn closing_inbox_stops_worker_even_while_another_read_holds_the_coordinator() {
    let (_temp, reads) = fixture();
    let held = reads.seen.lock().await;
    let (inbox, messages) = Inbox::channel();
    let link = super::super::feed_link::FeedLink::new();
    link.request_snapshot();
    let task =
        super::super::reseed::spawn_reseed(reads.clone(), link, inbox, Duration::from_secs(60));
    drop(messages);
    tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .unwrap()
        .unwrap();
    drop(held);
}

#[tokio::test]
async fn closing_observation_releases_an_explicit_refresh_waiting_for_the_coordinator() {
    use crate::application::session_observation::SessionObservation;
    use magistr_domain::identity::SessionAddress;
    use std::future::Future;

    let (_temp, reads) = fixture();
    let owner = Arc::new(SessionObservation::new(SessionAddress {
        project: "project".into(),
        workspace: "/workspace".into(),
        session: "selected".into(),
    }));
    owner.set_reader(reads.clone(), super::super::feed_link::FeedLink::new());
    let held = reads.seen.lock().await;
    let mut subscription = owner.subscribe();
    let refresh = subscription.refresh();
    tokio::pin!(refresh);
    let first_poll =
        std::future::poll_fn(|cx| std::task::Poll::Ready(refresh.as_mut().poll(cx))).await;
    assert!(first_poll.is_pending(), "refresh waits for the active read");

    owner.stop();
    let snapshot = tokio::time::timeout(Duration::from_secs(1), refresh)
        .await
        .unwrap()
        .unwrap();
    assert!(snapshot.closed);
    assert_eq!(snapshot.address.session.as_str(), "selected");
    drop(held);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalidation_during_a_read_suppresses_its_current_publication_until_the_retry() {
    let (_temp, reads) = fixture();
    let initial = fixtures::snapshot(vec![fixtures::chunk("selected")], Vec::new());
    let setup = initial.clone();
    reads.observe(None, true, move || setup).await;
    let (inbox, mut messages) = Inbox::channel();
    let (started, waiting) = tokio::sync::oneshot::channel();
    let (release, held) = std::sync::mpsc::channel();
    let delayed_reads = reads.clone();
    let delayed_inbox = inbox.clone();
    let delayed = tokio::spawn(async move {
        delayed_reads
            .observe(Some(&delayed_inbox), true, move || {
                started.send(()).unwrap();
                held.recv().unwrap();
                initial
            })
            .await
    });
    waiting.await.unwrap();
    assert!(reads.invalidate(|| true));
    release.send(()).unwrap();
    delayed.await.unwrap();
    assert!(
        messages.try_recv().is_err(),
        "an overtaken read cannot restore current"
    );
    reads
        .observe(Some(&inbox), true, || panic!("retry failed"))
        .await;
    let Some(Msg::Engine(EngineEvent::Seed(failed))) = messages.recv().await else {
        panic!("retry result");
    };
    assert!(matches!(failed.roadmap, ReadState::Stale { .. }));
    assert_eq!(
        failed.roadmap.current().unwrap_err().kind,
        ReadErrorKind::Interrupted
    );
    assert_eq!(
        failed.roadmap.last_successful().unwrap().value.chunks[0].id,
        "selected"
    );
    let recovered = fixtures::snapshot(vec![fixtures::chunk("selected")], Vec::new());
    reads.observe(Some(&inbox), true, move || recovered).await;
    let Some(Msg::Engine(EngineEvent::Seed(recovered))) = messages.recv().await else {
        panic!("recovery result");
    };
    assert!(recovered.roadmap.current().is_ok());
}
