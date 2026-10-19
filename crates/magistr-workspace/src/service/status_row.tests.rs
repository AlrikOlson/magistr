use super::*;
use crate::response::ResponseAdapter;

fn worker() -> WorkerStatus {
    WorkerStatus {
        pid: None,
        restarts: 0,
        consecutive_failures: 0,
        quarantined: 0,
        stopped: false,
        pending: 0,
    }
}

#[test]
fn status_reports_storage_and_owned_runtime_with_measured_accounting() {
    let root = Path::new("/tmp/workspace");
    let detection = NetworkDetection::detect(root, Some("0"));
    let response = StatusRow::new(root, &detection, "wal", worker(), IndexStatus::default())
        .respond(Vec::new());
    let sealed = ResponseAdapter::seal(&response).expect("sealed");
    let envelope = sealed.structured();
    assert_eq!(envelope["v"], 1);
    assert_eq!(envelope["ok"], true);
    assert_eq!(envelope["freshness"], "warm");
    let row = &envelope["results"][0];
    let measured = serde_json::to_string(row)
        .unwrap()
        .encode_utf16()
        .count()
        .div_ceil(4);
    assert_eq!(envelope["budget"]["used"], measured);
    assert_eq!(envelope["budget"]["requested"], measured);
    assert_eq!(row["root"], "/tmp/workspace");
    assert_eq!(row["journalMode"], "WAL");
    assert_eq!(row["appliedJournalMode"], "wal");
    assert_eq!(row["networkFs"], false);
    assert_eq!(row["detectionMethod"], "env-override");
    assert_eq!(row["envelope"], serde_json::json!({"min": 1, "latest": 1}));
    assert_eq!(row["worker"]["stopped"], false);
    assert_eq!(row["worker"]["quarantined"], 0);
    assert_eq!(row["index"]["generation"], 0);
    assert_eq!(
        envelope["notes"],
        serde_json::json!(IndexStatus::default().notes)
    );
}

#[test]
fn a_network_workspace_reports_truncate_and_the_notes_ride_the_envelope() {
    let root = Path::new("/mnt/share/workspace");
    let detection = NetworkDetection::detect(root, Some("1"));
    let response = StatusRow::new(
        root,
        &detection,
        "truncate",
        worker(),
        IndexStatus::default(),
    )
    .respond(vec!["degraded".into()]);
    let WorkspaceResponse::Success { results, notes, .. } = response else {
        panic!("status is a success envelope");
    };
    assert_eq!(results.rows()[0].journal_mode, "TRUNCATE");
    assert!(results.rows()[0].network_fs);
    let mut expected = vec!["degraded".to_owned()];
    expected.extend(IndexStatus::default().notes);
    assert_eq!(notes, Some(expected));
}

#[test]
fn blocked_inputs_and_index_errors_remain_visible_after_worker_recovery() {
    let root = Path::new("/tmp/workspace");
    let mut worker = worker();
    worker.pid = Some(4242);
    worker.restarts = 1;
    worker.quarantined = 2;
    let index = IndexStatus {
        freshness: Freshness::Warming,
        last_error: Some("parser unavailable".into()),
        notes: vec!["watcher is polling".into()],
        ..IndexStatus::default()
    };
    let response = StatusRow::new(
        root,
        &NetworkDetection::detect(root, Some("0")),
        "wal",
        worker,
        index,
    )
    .respond(Vec::new());
    let WorkspaceResponse::Success { results, notes, .. } = response else {
        panic!("status");
    };
    let row = &results.rows()[0];
    assert_eq!(row.worker.pid, Some(4242));
    assert_eq!(row.worker.consecutive_failures, 0);
    assert_eq!(row.index.freshness, "warming");
    let notes = notes.expect("runtime degradation");
    assert!(notes.iter().any(|note| note.contains("2 input(s) blocked")));
    assert!(notes.iter().any(|note| note.contains("parser unavailable")));
    assert!(notes.iter().any(|note| note.contains("watcher is polling")));
}
