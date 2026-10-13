use super::*;
use crate::config::inputs::ProjectInputs;

fn inputs(dir: &tempfile::TempDir) -> ProjectInputs {
    ProjectInputs::from_lookup(dir.path(), |key| {
        (key == "XDG_DATA_HOME").then(|| dir.path().join("data").display().to_string())
    })
}

#[tokio::test]
async fn no_provider_id_and_unknown_usage_survive_reopen_and_repeated_settlement() {
    let directory = tempfile::tempdir().unwrap();
    let inputs = inputs(&directory);
    let history = SessionHistory::new();
    history
        .begin(&inputs, directory.path(), "scripted", None)
        .await
        .unwrap();
    history.settle(SessionOutcome::Failed).await.unwrap();
    history.settle(SessionOutcome::Completed).await.unwrap();
    let path = storage::sessions_db_path(&inputs).unwrap();
    let conn = storage::open_db(&path).unwrap();
    let records =
        storage::query_sessions(&conn, &super::super::session_filter::SessionFilter::new())
            .unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].outcome, SessionOutcome::Failed);
    assert_eq!(records[0].provider_session_id, None);
    assert_eq!(records[0].cost_usd, None);
    assert_eq!(records[0].model, None);
    assert!(!records[0].can_resume());
}

#[tokio::test]
async fn reconnect_retains_identity_and_counts_new_terminal_responses_once() {
    let directory = tempfile::tempdir().unwrap();
    let inputs = inputs(&directory);
    let history = SessionHistory::new();
    history
        .begin(&inputs, directory.path(), "scripted", None)
        .await
        .unwrap();
    history.finished_turn();
    history.settle(SessionOutcome::Completed).await.unwrap();
    let path = storage::sessions_db_path(&inputs).unwrap();
    let conn = storage::open_db(&path).unwrap();
    let first = storage::query_sessions(&conn, &super::super::session_filter::SessionFilter::new())
        .unwrap()
        .remove(0);
    history
        .begin(&inputs, directory.path(), "scripted", None)
        .await
        .unwrap();
    history.finished_turn();
    history.settle(SessionOutcome::Cancelled).await.unwrap();
    let second = storage::get_session(&conn, &first.session_id)
        .unwrap()
        .unwrap();
    assert_eq!(storage::count_sessions(&conn).unwrap(), 1);
    assert_eq!(second.started_at, first.started_at);
    assert_eq!(second.turns, Some(2));
    assert_eq!(second.outcome, SessionOutcome::Cancelled);
}

#[tokio::test]
async fn failed_storage_is_returned_after_settlement_without_replacing_facts() {
    let directory = tempfile::tempdir().unwrap();
    let inputs = inputs(&directory);
    let path = storage::sessions_db_path(&inputs).unwrap();
    std::fs::create_dir_all(&path).unwrap();
    let history = SessionHistory::new();
    history
        .begin(&inputs, directory.path(), "scripted", None)
        .await
        .unwrap();
    assert!(history.settle(SessionOutcome::Completed).await.is_err());
    let state = history.0.lock().unwrap();
    assert_eq!(
        state.record.as_ref().unwrap().outcome,
        SessionOutcome::Completed
    );
    assert!(
        state.started.is_none(),
        "the owner settled even when the store refused"
    );
}

#[tokio::test]
async fn text_is_bounded_and_prior_connection_replay_does_not_duplicate_summary() {
    use agent_client_protocol::schema::v1::ContentChunk;
    let directory = tempfile::tempdir().unwrap();
    let history = SessionHistory::new();
    history
        .begin(&inputs(&directory), directory.path(), "scripted", None)
        .await
        .unwrap();
    let text = AcpEvent::SessionUpdate {
        session_id: "provider".into(),
        update: Box::new(SessionUpdate::AgentMessageChunk(ContentChunk::new(
            ContentBlock::from("é".repeat(3000)),
        ))),
    };
    history.observe(&text);
    assert!(
        history
            .0
            .lock()
            .unwrap()
            .record
            .as_ref()
            .unwrap()
            .summary
            .is_none()
    );
    history.begin_turn();
    history.observe(&text);
    history.finished_turn();
    let state = history.0.lock().unwrap();
    assert_eq!(
        state
            .record
            .as_ref()
            .unwrap()
            .summary
            .as_ref()
            .unwrap()
            .chars()
            .count(),
        SUMMARY_CHARS
    );
}
