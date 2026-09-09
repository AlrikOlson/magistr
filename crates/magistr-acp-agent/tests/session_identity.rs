//! Runtime-selected session IDs and models through the shared ACP agent.

mod fake_client;

use agent_client_protocol::BoxFuture;
use agent_client_protocol::schema::v1::{
    ContentBlock, ContentChunk, LoadSessionRequest, SessionId, SessionUpdate, StopReason,
    TextContent,
};
use fake_client::{PermissionAnswer, initialize, new_session, prompt_and_wait, with_provider};
use magistr_acp_agent::{AgentError, ModelChoice, Provider, SessionOpened, SessionStart, Turn};

const RUNTIME_ID: &str = "runtime-thread";
const DEFAULT_MODEL: &str = "default-model";
const RESTORED_MODEL: &str = "restored-model";

/// Answers a fixed runtime identity and echoes the identity the turn carries.
struct RuntimeProvider {
    model: Option<&'static str>,
}

impl Provider for RuntimeProvider {
    fn name(&self) -> &str {
        "runtime-test"
    }

    fn models(&self) -> Vec<ModelChoice> {
        vec![
            ModelChoice::new(DEFAULT_MODEL, "Default"),
            ModelChoice::new(RESTORED_MODEL, "Restored"),
        ]
    }

    fn start(&self, _session: SessionStart) -> BoxFuture<'_, Result<SessionOpened, AgentError>> {
        Box::pin(async move {
            let mut opened = SessionOpened::new(SessionId::new(RUNTIME_ID)).replay(vec![
                SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                    TextContent::new("replay;"),
                ))),
            ]);
            opened.model = self.model.map(str::to_owned);
            Ok(opened)
        })
    }

    fn prompt(&self, turn: Turn) -> BoxFuture<'_, Result<StopReason, AgentError>> {
        Box::pin(async move {
            turn.message(format!(
                "{}:{}",
                turn.session_id(),
                turn.model().unwrap_or("no-model")
            ))?;
            Ok(StopReason::EndTurn)
        })
    }
}

#[tokio::test]
async fn new_session_uses_the_runtime_identity_for_replay_and_the_next_turn() {
    let (response, observed) = with_provider(
        RuntimeProvider { model: None },
        PermissionAnswer::Cancel,
        async |cx, observed| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            assert_eq!(session.session_id.to_string(), RUNTIME_ID);
            assert_eq!(
                observed.messages(),
                "replay;",
                "replay precedes the response"
            );
            prompt_and_wait(&cx, &session.session_id, "continue").await
        },
    )
    .await;
    assert_eq!(response.stop_reason, StopReason::EndTurn);
    assert_eq!(
        observed.messages(),
        format!("replay;{RUNTIME_ID}:{DEFAULT_MODEL}")
    );
    assert!(
        observed
            .updates
            .lock()
            .expect("updates")
            .iter()
            .all(|(id, _)| id.to_string() == RUNTIME_ID)
    );
}

#[tokio::test]
async fn new_session_preserves_the_runtime_model_in_the_picker_and_next_turn() {
    let (_, observed) = with_provider(
        RuntimeProvider {
            model: Some(RESTORED_MODEL),
        },
        PermissionAnswer::Cancel,
        async |cx, _| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            let picker = serde_json::to_value(session.config_options).expect("picker JSON");
            assert_eq!(picker[0]["currentValue"], RESTORED_MODEL);
            prompt_and_wait(&cx, &session.session_id, "continue").await
        },
    )
    .await;
    assert_eq!(
        observed.messages(),
        format!("replay;{RUNTIME_ID}:{RESTORED_MODEL}")
    );
}

#[tokio::test]
async fn load_session_preserves_the_restored_model_in_the_picker_and_next_turn() {
    let (_, observed) = with_provider(
        RuntimeProvider {
            model: Some(RESTORED_MODEL),
        },
        PermissionAnswer::Cancel,
        async |cx, _| {
            initialize(&cx).await?;
            let session_id = SessionId::new(RUNTIME_ID);
            let loaded = cx
                .send_request(LoadSessionRequest::new(
                    session_id.clone(),
                    std::env::temp_dir(),
                ))
                .block_task()
                .await?;
            let picker = serde_json::to_value(loaded.config_options).expect("picker JSON");
            assert_eq!(picker[0]["currentValue"], RESTORED_MODEL);
            prompt_and_wait(&cx, &session_id, "continue").await
        },
    )
    .await;
    assert_eq!(
        observed.messages(),
        format!("replay;{RUNTIME_ID}:{RESTORED_MODEL}")
    );
}

#[tokio::test]
async fn an_unknown_runtime_model_refuses_open_without_replay_or_registration() {
    let (errors, observed) = with_provider(
        RuntimeProvider {
            model: Some("unavailable-model"),
        },
        PermissionAnswer::Cancel,
        async |cx, _| {
            initialize(&cx).await?;
            let opening = new_session(&cx).await.expect_err("model is not offered");
            let prompting = prompt_and_wait(&cx, &SessionId::new(RUNTIME_ID), "continue")
                .await
                .expect_err("failed open did not register the session");
            Ok((opening, prompting))
        },
    )
    .await;
    assert_eq!(
        errors.0.data,
        Some(serde_json::json!("unknown model: unavailable-model"))
    );
    assert_eq!(
        errors.1.data,
        Some(serde_json::json!(format!("unknown session: {RUNTIME_ID}")))
    );
    assert!(observed.messages().is_empty());
}

#[tokio::test]
async fn load_rejects_a_different_runtime_identity_without_replay_or_registration() {
    let (errors, observed) = with_provider(
        RuntimeProvider { model: None },
        PermissionAnswer::Cancel,
        async |cx, _| {
            initialize(&cx).await?;
            let loading = cx
                .send_request(LoadSessionRequest::new(
                    SessionId::new("requested-thread"),
                    std::env::temp_dir(),
                ))
                .block_task()
                .await
                .expect_err("runtime restored a different thread");
            let prompting = prompt_and_wait(&cx, &SessionId::new(RUNTIME_ID), "continue")
                .await
                .expect_err("mismatched identity was not registered");
            Ok((loading, prompting))
        },
    )
    .await;
    assert_eq!(
        errors.0.data,
        Some(serde_json::json!(format!(
            "session/load named requested-thread but the provider restored {RUNTIME_ID}"
        )))
    );
    assert_eq!(
        errors.1.data,
        Some(serde_json::json!(format!("unknown session: {RUNTIME_ID}")))
    );
    assert!(observed.messages().is_empty());
}

#[tokio::test]
async fn a_runtime_identity_collision_keeps_the_existing_session_usable() {
    let (error, observed) = with_provider(
        RuntimeProvider { model: None },
        PermissionAnswer::Cancel,
        async |cx, _| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            let collision = new_session(&cx)
                .await
                .expect_err("runtime identity already open");
            prompt_and_wait(&cx, &session.session_id, "still open").await?;
            Ok(collision)
        },
    )
    .await;
    assert_eq!(
        error.data,
        Some(serde_json::json!(format!(
            "session already open: {RUNTIME_ID}"
        )))
    );
    assert_eq!(
        observed.messages(),
        format!("replay;{RUNTIME_ID}:{DEFAULT_MODEL}")
    );
}
