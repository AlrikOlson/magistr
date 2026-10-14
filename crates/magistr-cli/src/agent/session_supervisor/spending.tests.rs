use super::*;
use crate::agent::persistence::{SessionHistory, session_filter::SessionFilter, storage};
use crate::config::inputs::ProjectInputs;

fn inputs(directory: &tempfile::TempDir) -> ProjectInputs {
    ProjectInputs::from_lookup(directory.path(), |key| match key {
        "XDG_DATA_HOME" => Some(directory.path().join("data").display().to_string()),
        "MAGISTR_USER" => Some("spending-author".into()),
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

fn ledger(inputs: &ProjectInputs) -> Vec<crate::cost::CostEntry> {
    crate::cost::storage::load_entries(&crate::cost::costs_path::costs_path(inputs).unwrap())
        .unwrap()
}

fn price(message: AgentMessage) -> Option<f64> {
    let AgentMessage::Result { total_cost_usd, .. } = message else {
        panic!("expected a completed turn");
    };
    total_cost_usd
}

#[tokio::test]
async fn terminal_and_live_usage_keep_reported_spending_across_unpriced_turns() {
    let directory = tempfile::tempdir().unwrap();
    let inputs = inputs(&directory);
    let mut fixture = start("", &inputs, &SessionHistory::new()).await;
    let (sender, mut live) = crate::agent::observation::channel(ObservationCapacity::DEFAULT);
    fixture.supervisor.inner.lock().unwrap().cockpit_events = Some(sender);
    for prompt in ["priced", "unpriced usage"] {
        fixture.supervisor.send_followup(&prompt.into()).unwrap();
        assert_eq!(price(fixture.next_terminal().await.unwrap()), Some(0.25));
        let mut reported = None;
        loop {
            match tokio::time::timeout(DEADLINE, live.recv())
                .await
                .unwrap()
                .unwrap()
            {
                AgentEvent::Usage { cost_cents, .. } => reported = cost_cents,
                AgentEvent::TurnEnded(_) => break,
                _ => {}
            }
        }
        assert_eq!(
            reported,
            Some(25),
            "the live projection keeps the same price"
        );
    }
    fixture.close().await;
    let entries = ledger(&inputs);
    assert_eq!(entries.len(), 1);
    assert_eq!(serde_json::to_value(&entries[0]).unwrap()["cost_usd"], 0.25);
    assert_eq!(entries[0].turns, 2);
}

#[tokio::test]
async fn cumulative_spending_replaces_one_conversation_across_reconnect_and_resume() {
    let directory = tempfile::tempdir().unwrap();
    let inputs = inputs(&directory);
    let history = SessionHistory::new();
    let mut first = start("priced", &inputs, &history).await;
    assert_eq!(price(first.next_terminal().await.unwrap()), Some(0.25));
    first.supervisor.send_followup(&"higher".into()).unwrap();
    assert_eq!(price(first.next_terminal().await.unwrap()), Some(0.75));
    first.close().await;
    let conn = storage::open_db(&storage::sessions_db_path(&inputs).unwrap()).unwrap();
    let record = storage::query_sessions(&conn, &SessionFilter::new())
        .unwrap()
        .remove(0);
    drop(conn);
    let mut reconnected = start("unpriced usage", &inputs, &history).await;
    assert_eq!(
        price(reconnected.next_terminal().await.unwrap()),
        Some(0.75)
    );
    reconnected.close().await;
    let conn = storage::open_db(&storage::sessions_db_path(&inputs).unwrap()).unwrap();
    let current = storage::get_session(&conn, &record.session_id)
        .unwrap()
        .unwrap();
    drop(conn);
    let mut resumed = start(
        "unpriced usage",
        &inputs,
        &SessionHistory::from_record(current),
    )
    .await;
    assert_eq!(price(resumed.next_terminal().await.unwrap()), Some(0.75));
    resumed.close().await;
    let entries = ledger(&inputs);
    assert_eq!(
        entries.len(),
        1,
        "a cumulative snapshot is never appended as a new charge"
    );
    assert_eq!(serde_json::to_value(&entries[0]).unwrap()["cost_usd"], 0.75);
    assert_eq!(entries[0].turns, 4);
}

#[tokio::test]
async fn reported_zero_and_unreported_spending_remain_distinct_in_the_ledger() {
    let directory = tempfile::tempdir().unwrap();
    let inputs = inputs(&directory);
    for (prompt, expected) in [("zero", Some(0.0)), ("unpriced usage", None)] {
        let mut fixture = start(prompt, &inputs, &SessionHistory::new()).await;
        assert_eq!(price(fixture.next_terminal().await.unwrap()), expected);
        fixture.close().await;
    }
    let entries = ledger(&inputs);
    assert_eq!(entries.len(), 2);
    assert_eq!(serde_json::to_value(&entries[0]).unwrap()["cost_usd"], 0.0);
    assert!(serde_json::to_value(&entries[1]).unwrap()["cost_usd"].is_null());
}

#[tokio::test]
async fn provider_failure_after_a_price_keeps_the_reported_charge() {
    let directory = tempfile::tempdir().unwrap();
    let inputs = inputs(&directory);
    let mut fixture = start("priced fail", &inputs, &SessionHistory::new()).await;
    assert!(fixture.next_terminal().await.is_err());
    fixture.close().await;
    let entries = ledger(&inputs);
    assert_eq!(entries.len(), 1);
    assert_eq!(serde_json::to_value(&entries[0]).unwrap()["cost_usd"], 0.5);
}

#[tokio::test]
async fn restored_live_spending_is_published_before_any_new_turn() {
    let directory = tempfile::tempdir().unwrap();
    let inputs = inputs(&directory);
    let history = SessionHistory::new();
    let mut first = start("priced", &inputs, &history).await;
    first.next_terminal().await.unwrap();
    first.close().await;
    let conn = storage::open_db(&storage::sessions_db_path(&inputs).unwrap()).unwrap();
    let record = storage::query_sessions(&conn, &SessionFilter::new())
        .unwrap()
        .remove(0);
    drop(conn);
    let restored = SessionHistory::from_record(record);
    restored
        .begin(&inputs, directory.path(), "supervisor-test", None)
        .await
        .unwrap();
    let (client, agent) = agent_client_protocol::Channel::duplex();
    let mut server = tokio::spawn(async move {
        magistr_acp_agent::serve(ScriptedProvider, agent)
            .await
            .unwrap();
    });
    let mut transport = magistr_acp::AcpTransport::new_with_args(
        std::path::PathBuf::from("unused-in-process-agent"),
        Vec::new(),
        Some(directory.path().to_path_buf()),
    )
    .unwrap();
    transport.connect_over(client).await.unwrap();
    let (log, _reader) = mpsc::unbounded_channel();
    let (mut supervisor, mut events) = SessionSupervisor::start_interactive_bounded(
        transport,
        magistr_application::sessions::SessionOptions::new(),
        Box::new(TitledPolicy(log)),
        ObservationCapacity::DEFAULT,
        Some(restored),
    )
    .unwrap();
    let price = tokio::time::timeout(DEADLINE, async {
        while let Some(event) = events.recv().await {
            if let AgentEvent::Usage {
                context,
                cost_cents,
                ..
            } = event
            {
                assert!(
                    context.is_none(),
                    "restored spending has no new context reading"
                );
                return cost_cents;
            }
        }
        None
    })
    .await;
    supervisor.terminate_session().await.unwrap();
    tokio::time::timeout(DEADLINE, &mut server)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        price.expect("restored price is available while idle"),
        Some(25)
    );
}
