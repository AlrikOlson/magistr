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

#[test]
fn missing_foreign_and_invalid_prices_cannot_erase_a_reported_amount() {
    use agent_client_protocol::schema::v1::{Cost, UsageUpdate};
    let history = SessionHistory::new();
    let observe = |cost| {
        history.observe(&AcpEvent::SessionUpdate {
            session_id: "provider".into(),
            update: Box::new(SessionUpdate::UsageUpdate(
                UsageUpdate::new(5, 100).cost(cost),
            )),
        });
    };
    assert_eq!(history.reported_cost_usd(), None);
    observe(Some(Cost::new(0.25, "USD")));
    for cost in [
        None,
        Some(Cost::new(10.0, "EUR")),
        Some(Cost::new(-1.0, "USD")),
        Some(Cost::new(f64::NAN, "USD")),
        Some(Cost::new(f64::INFINITY, "USD")),
    ] {
        observe(cost);
        assert_eq!(history.reported_cost_usd(), Some(0.25));
    }
    observe(Some(Cost::new(0.0, "USD")));
    assert_eq!(history.reported_cost_usd(), Some(0.0));
}
#[tokio::test]
async fn a_name_is_saved_by_the_owner_and_survives_reconnect_and_resume() {
    let directory = tempfile::tempdir().unwrap();
    let inputs = inputs(&directory);
    let history = SessionHistory::new();
    history
        .begin(&inputs, directory.path(), "scripted", None)
        .await
        .unwrap();
    history.rename("Conversation about café").unwrap();
    assert!(history.rename(" ").is_err());
    assert!(history.rename(&"x".repeat(121)).is_err());
    history.settle(SessionOutcome::Completed).await.unwrap();
    history
        .begin(&inputs, directory.path(), "scripted", None)
        .await
        .unwrap();
    assert_eq!(
        history.conversation().unwrap().name.as_deref(),
        Some("Conversation about café")
    );
    history.rename("Reviewed changes").unwrap();
    history.settle(SessionOutcome::Cancelled).await.unwrap();
    let conn = storage::open_db(&storage::sessions_db_path(&inputs).unwrap()).unwrap();
    let records =
        storage::query_sessions(&conn, &super::super::session_filter::SessionFilter::new())
            .unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].name.as_deref(), Some("Reviewed changes"));
    let resumed = SessionHistory::from_record(records[0].clone());
    assert_eq!(resumed.conversation().unwrap().name, records[0].name);
}
