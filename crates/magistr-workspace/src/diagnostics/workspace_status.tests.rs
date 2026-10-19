use super::*;
use crate::contracts::Freshness as IndexFreshness;

#[test]
fn status_carries_journal_mode_versions_and_snapshot_notes() {
    let dir = tempfile::tempdir().expect("workspace");
    let config = ResolvedConfig::default();
    let index = IndexStatus {
        freshness: IndexFreshness::Warming,
        last_error: Some("worker deadline".into()),
        ..IndexStatus::default()
    };
    let worker = WorkerStatus {
        pid: Some(4242),
        restarts: 1,
        consecutive_failures: 2,
        quarantined: 1,
        stopped: false,
        pending: 0,
    };
    let status = WorkspaceStatus::observe(
        dir.path(),
        &dir.path().join(".magistr/workspace"),
        &config,
        Some("1"),
        Some(worker),
        Some(index),
    );
    assert_eq!(status.journal_mode, "TRUNCATE");
    assert_eq!(status.network_detection.method, "env-override");
    assert_eq!(
        (
            status.envelope_versions.min,
            status.envelope_versions.latest
        ),
        (1, 1)
    );
    assert_eq!(
        status.worker.as_ref().map(|worker| worker.pid),
        Some(Some(4242))
    );
    assert_eq!(
        status.index.as_ref().map(|index| index.freshness),
        Some("warming")
    );
    assert!(
        status
            .notes
            .iter()
            .any(|note| note.contains("network filesystem"))
    );
    assert!(
        status
            .notes
            .iter()
            .any(|note| note.contains("2 consecutive failure(s)"))
    );
    assert!(
        status
            .notes
            .iter()
            .any(|note| note.contains("worker deadline"))
    );
    let wire = serde_json::to_value(&status).expect("json");
    assert_eq!(wire["journalMode"], "TRUNCATE");
    assert_eq!(wire["envelopeVersions"]["latest"], 1);
    let WorkspaceResponse::Success { notes, results, .. } = status.respond(4000) else {
        panic!("status is a success envelope");
    };
    assert_eq!(results.rows().len(), 1);
    assert!(notes.expect("degradations ride the notes").len() >= 3);
}

#[test]
fn a_local_healthy_workspace_has_no_notes() {
    let dir = tempfile::tempdir().expect("workspace");
    let status = WorkspaceStatus::observe(
        dir.path(),
        &dir.path().join(".magistr/workspace"),
        &ResolvedConfig::default(),
        Some("0"),
        None,
        None,
    );
    assert_eq!(status.journal_mode, "WAL");
    assert!(status.notes.is_empty());
    assert!(status.worker.is_none() && status.index.is_none());
}
