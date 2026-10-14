use super::*;
use crate::agent::persistence::{
    SessionHistory, session_filter::SessionFilter, session_outcome::SessionOutcome, storage,
};
use crate::config::inputs::ProjectInputs;

fn inputs(directory: &tempfile::TempDir) -> ProjectInputs {
    ProjectInputs::from_lookup(directory.path(), |key| match key {
        "XDG_DATA_HOME" => Some(directory.path().join("data").display().to_string()),
        "MAGISTR_USER" => Some("history-author".into()),
        _ => None,
    })
}

async fn start(prompt: &str, inputs: &ProjectInputs, history: &SessionHistory) -> Fixture {
    Fixture::start_recording(
        prompt,
        ObservationCapacity::DEFAULT,
        None,
        Some((history.clone(), inputs.clone())),
    )
    .await
}

#[tokio::test]
async fn actual_completion_reopens_with_usage_summary_and_one_identity_across_reconnect() {
    let directory = tempfile::tempdir().unwrap();
    let inputs = inputs(&directory);
    let history =
        SessionHistory::for_purpose(magistr_application::sessions::SessionPurpose::Roadmap);
    let mut first = start("priced", &inputs, &history).await;
    first.next_terminal().await.unwrap();
    first.supervisor.send_followup(&"second".into()).unwrap();
    first.next_terminal().await.unwrap();
    first.close().await;
    first.supervisor.terminate_session().await.unwrap();
    let path = storage::sessions_db_path(&inputs).unwrap();
    let conn = storage::open_db(&path).unwrap();
    let before = storage::query_sessions(&conn, &SessionFilter::new())
        .unwrap()
        .remove(0);
    assert_eq!(
        before.purpose,
        magistr_application::sessions::SessionPurpose::Roadmap
    );
    assert_eq!(before.outcome, Some(SessionOutcome::Completed));
    assert_eq!(before.turns, Some(2));
    assert_eq!(before.cost_usd, Some(0.25));
    assert_eq!(before.model.as_deref(), Some(ScriptedProvider::SLOW));
    assert_eq!(before.owner.as_deref(), Some("history-author"));
    assert!(before.provider_session_id.is_some());
    assert_eq!(
        before.summary.as_deref(),
        Some("started: secondfinished: second")
    );
    storage::tag_session(&conn, &before.session_id, "reviewed").unwrap();
    storage::set_session_owner(&conn, &before.session_id, "next-person").unwrap();
    drop(conn);
    let restored_history = SessionHistory::from_record(before.clone());
    assert_eq!(restored_history.id(), history.id());
    let mut restored = start("third", &inputs, &restored_history).await;
    restored.next_terminal().await.unwrap();
    restored.close().await;
    let conn = storage::open_db(&path).unwrap();
    let after = storage::get_session(&conn, &before.session_id)
        .unwrap()
        .unwrap();
    assert_eq!(storage::count_sessions(&conn).unwrap(), 1);
    assert_eq!(after.purpose, before.purpose);
    assert_eq!(after.started_at, before.started_at);
    assert_eq!(after.turns, Some(3));
    assert_eq!(after.owner.as_deref(), Some("next-person"));
    assert_eq!(after.tags, ["reviewed"]);
    assert_eq!(after.cost_usd, Some(0.25));
}

#[tokio::test]
async fn failed_provider_session_is_saved_with_its_actual_failure() {
    let directory = tempfile::tempdir().unwrap();
    let inputs = inputs(&directory);
    let mut fixture = start("fail", &inputs, &SessionHistory::new()).await;
    assert!(fixture.next_terminal().await.is_err());
    fixture.close().await;
    let conn = storage::open_db(&storage::sessions_db_path(&inputs).unwrap()).unwrap();
    let record = storage::query_sessions(&conn, &SessionFilter::new())
        .unwrap()
        .remove(0);
    assert_eq!(record.outcome, Some(SessionOutcome::Failed));
    assert_eq!(record.cost_usd, None);
    assert_eq!(record.turns, Some(0));
    assert!(record.provider_session_id.is_some());
}

#[tokio::test]
async fn failed_store_reports_a_diagnostic_and_keeps_success_and_cleanup() {
    let directory = tempfile::tempdir().unwrap();
    let inputs = inputs(&directory);
    std::fs::create_dir_all(storage::sessions_db_path(&inputs).unwrap()).unwrap();
    let mut fixture = start("done", &inputs, &SessionHistory::new()).await;
    fixture.next_terminal().await.unwrap();
    fixture.close().await;
    assert_eq!(
        fixture
            .supervisor
            .snapshot()
            .last_outcome
            .unwrap()
            .stop_reason,
        Some(StopReason::EndTurn)
    );
    let mut reported = false;
    while let Some(message) = fixture.messages.recv().await {
        if matches!(message, Ok(AgentMessage::System { subtype, .. }) if subtype == "history_save_failed")
        {
            reported = true;
        }
    }
    assert!(
        reported,
        "the owner reports storage failure separately from the terminal result"
    );
}

#[tokio::test]
async fn forced_owner_close_records_stopped_after_releasing_the_connection() {
    let directory = tempfile::tempdir().unwrap();
    let inputs = inputs(&directory);
    let mut fixture = start("hold active", &inputs, &SessionHistory::new()).await;
    fixture.next_started().await;
    fixture.supervisor.abort();
    tokio::time::timeout(DEADLINE, &mut fixture.server)
        .await
        .unwrap()
        .unwrap();
    let path = storage::sessions_db_path(&inputs).unwrap();
    let record = tokio::time::timeout(DEADLINE, async {
        loop {
            let conn = storage::open_db(&path).unwrap();
            let records = storage::query_sessions(&conn, &SessionFilter::new()).unwrap();
            if let Some(record) = records.into_iter().next() {
                return record;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(record.outcome, Some(SessionOutcome::Stopped));
    assert_eq!(record.turns, Some(0));
}

#[cfg(unix)]
#[tokio::test]
async fn a_real_child_that_fails_setup_saves_without_a_provider_id() {
    let directory = tempfile::tempdir().unwrap();
    let inputs = inputs(&directory);
    let runtime = crate::agent::AgentRuntime {
        path: "/usr/bin/false".into(),
        args: Vec::new(),
        auth: crate::agent::AuthMethod::Implicit,
        provider: "failed-agent".into(),
    };
    let spec = crate::agent::ResolvedSessionSpec::resolve_with_runtime(
        crate::agent::AgentConfig::new("do not infer", directory.path()).with_inputs(&inputs),
        &crate::config::user_config::UserConfig::default(),
        runtime,
    )
    .unwrap();
    assert!(SessionSupervisor::start(spec).await.is_err());
    let conn = storage::open_db(&storage::sessions_db_path(&inputs).unwrap()).unwrap();
    let record = storage::query_sessions(&conn, &SessionFilter::new())
        .unwrap()
        .remove(0);
    assert_eq!(record.outcome, Some(SessionOutcome::Failed));
    assert_eq!(record.provider_session_id, None);
    assert_eq!(record.cost_usd, None);
    assert!(!record.can_resume());
}

#[tokio::test]
async fn a_busy_store_reports_within_the_close_budget_after_provider_cleanup() {
    let directory = tempfile::tempdir().unwrap();
    let inputs = inputs(&directory);
    let conn = storage::open_db(&storage::sessions_db_path(&inputs).unwrap()).unwrap();
    conn.execute_batch("BEGIN IMMEDIATE;").unwrap();
    let mut fixture = start("done", &inputs, &SessionHistory::new()).await;
    fixture.next_terminal().await.unwrap();
    let started = std::time::Instant::now();
    fixture.close().await;
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(
        fixture
            .supervisor
            .snapshot()
            .last_outcome
            .unwrap()
            .stop_reason,
        Some(StopReason::EndTurn)
    );
    assert_eq!(storage::count_sessions(&conn).unwrap(), 0);
    conn.execute_batch("ROLLBACK;").unwrap();
}

#[tokio::test]
async fn immediate_idle_close_keeps_the_already_accepted_provider_identity() {
    let directory = tempfile::tempdir().unwrap();
    let inputs = inputs(&directory);
    let mut fixture = start("", &inputs, &SessionHistory::new()).await;
    fixture.close().await;
    let conn = storage::open_db(&storage::sessions_db_path(&inputs).unwrap()).unwrap();
    let record = storage::query_sessions(&conn, &SessionFilter::new())
        .unwrap()
        .remove(0);
    assert!(
        record.provider_session_id.is_some(),
        "connect already accepted the provider session before owner observation began"
    );
    assert_eq!(record.outcome, Some(SessionOutcome::Stopped));
}

#[tokio::test]
async fn saved_planning_reopens_under_yolo_and_refuses_implementation_on_the_wire() {
    use crate::agent::{AgentConfig, AgentRuntime, AuthMethod, ResolvedSessionSpec};
    use crate::permissions::PermissionProfile;
    use magistr_application::sessions::SessionPurpose;

    let directory = tempfile::tempdir().unwrap();
    let inputs = inputs(&directory);
    let mut config = AgentConfig::new("", directory.path())
        .with_inputs(&inputs)
        .permission_profile(PermissionProfile::Yolo);
    config.history = SessionHistory::for_purpose(SessionPurpose::Roadmap);
    let id = config.history.id();

    for (attempt, prompt, expected) in [
        (0, "mcp__magistr__roadmap_add_chunk", "selected allow"),
        (1, "Write", "selected reject"),
    ] {
        if attempt != 0 {
            config = crate::agent::persistence::resume::saved_config(
                AgentConfig::new("", directory.path())
                    .with_inputs(&inputs)
                    .permission_profile(PermissionProfile::Yolo),
                &id,
            )
            .unwrap();
            assert_eq!(config.history.id(), id);
            assert_eq!(config.history.purpose(), SessionPurpose::Roadmap);
            assert_eq!(config.resume.as_deref(), Some("parity-session"));
        }
        let spec = ResolvedSessionSpec::resolve_with_runtime(
            config.clone(),
            &crate::config::user_config::UserConfig::default(),
            AgentRuntime {
                path: "unused-in-process-agent".into(),
                args: Vec::new(),
                auth: AuthMethod::Implicit,
                provider: "permission-fixture".into(),
            },
        )
        .unwrap();
        let history = spec.history();
        // Channel fixtures initialize the same durable owner before connecting.
        history
            .begin(
                &inputs,
                spec.cwd(),
                spec.runtime().provider_id(),
                spec.model(),
            )
            .await
            .unwrap();
        let (transport, mut server, mut outcomes) =
            permission_agent_with_mode(spec.cwd(), spec.session_mode()).await;
        let policy = spec.into_session_policy();
        let (mut supervisor, mut messages) =
            SessionSupervisor::start_connected_history(prompt, transport, Some(policy), history)
                .await
                .unwrap();
        let outcome = tokio::time::timeout(DEADLINE, outcomes.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(wire(outcome), expected);
        tokio::time::timeout(DEADLINE, async {
            while let Some(message) = messages.recv().await {
                if matches!(message.unwrap(), AgentMessage::Result { .. }) {
                    return;
                }
            }
            panic!("the owner must observe the actual terminal response");
        })
        .await
        .unwrap();
        supervisor.terminate_session().await.unwrap();
        tokio::time::timeout(DEADLINE, &mut server)
            .await
            .unwrap()
            .unwrap();

        let conn = storage::open_readonly_db(&storage::sessions_db_path(&inputs).unwrap()).unwrap();
        assert_eq!(storage::count_sessions(&conn).unwrap(), 1);
        let saved = storage::get_session(&conn, &id).unwrap().unwrap();
        assert_eq!(saved.purpose, SessionPurpose::Roadmap);
        assert!(saved.can_resume());
        assert_eq!(saved.provider_session_id.as_deref(), Some("parity-session"));
        assert_eq!(saved.turns, Some(attempt + 1));
    }
}
