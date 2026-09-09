//! The agent against a scripted in-process client: handshake, a full turn,
//! the permission round-trip, cancel mid-turn (and the queue behind it),
//! session/load, the model switch, and the refusals.

mod fake_client;

use std::time::Duration;

use agent_client_protocol::schema::v1::{
    AuthenticateRequest, CancelNotification, LoadSessionRequest, RequestPermissionOutcome,
    SessionConfigOptionValue, SessionId, SessionUpdate, SetSessionConfigOptionRequest, StopReason,
    ToolCallStatus,
};
use fake_client::{
    PermissionAnswer, initialize, new_session, prompt, prompt_and_wait, with_client,
};
use magistr_acp_agent::ScriptedProvider;

#[tokio::test]
async fn initialize_advertises_load_session_the_agent_and_the_models() {
    let (init, _) = with_client(PermissionAnswer::Cancel, async |cx, _| {
        initialize(&cx).await
    })
    .await;
    assert!(init.agent_capabilities.load_session);
    assert!(init.agent_capabilities.prompt_capabilities.embedded_context);
    assert!(init.auth_methods.is_empty(), "no auth methods to select");
    let info = init.agent_info.expect("agent_info");
    assert_eq!(info.name, "magistr");
    let meta = init.meta.expect("_meta.magistr");
    let magistr = &meta["magistr"];
    assert_eq!(magistr["provider"], ScriptedProvider::NAME);
    assert_eq!(magistr["models"][0]["id"], ScriptedProvider::FAST);
    assert_eq!(magistr["models"][1]["id"], ScriptedProvider::SLOW);
}

#[tokio::test]
async fn authenticate_refuses_a_method_that_was_never_advertised() {
    let (err, _) = with_client(PermissionAnswer::Cancel, async |cx, _| {
        initialize(&cx).await?;
        Ok(cx
            .send_request(AuthenticateRequest::new("github"))
            .block_task()
            .await
            .expect_err("no auth method is advertised"))
    })
    .await;
    assert_eq!(
        err.data,
        Some(serde_json::json!("unknown auth method: github"))
    );
}

#[tokio::test]
async fn a_full_turn_streams_thought_message_and_usage_then_ends() {
    let (response, observed) = with_client(PermissionAnswer::Cancel, async |cx, _| {
        initialize(&cx).await?;
        let session = new_session(&cx).await?;
        let picker = session.config_options.expect("the model picker");
        assert_eq!(picker[0].id.to_string(), "model");
        prompt_and_wait(&cx, &session.session_id, "hello there").await
    })
    .await;
    assert_eq!(response.stop_reason, StopReason::EndTurn);
    assert!(observed.saw(|u| matches!(u, SessionUpdate::AgentThoughtChunk(_))));
    assert_eq!(
        observed.messages(),
        format!("[{}] echo: hello there", ScriptedProvider::FAST)
    );
    assert!(observed.saw(|u| matches!(u, SessionUpdate::UsageUpdate(usage) if usage.used == 11)));
}

#[tokio::test]
async fn the_plan_is_published_on_request() {
    let (_, observed) = with_client(PermissionAnswer::Cancel, async |cx, _| {
        initialize(&cx).await?;
        let session = new_session(&cx).await?;
        prompt_and_wait(&cx, &session.session_id, "make a plan").await
    })
    .await;
    assert!(observed.saw(|u| matches!(u, SessionUpdate::Plan(plan) if plan.entries.len() == 2)));
}

#[tokio::test]
async fn a_permission_round_trip_reaches_the_client_and_its_answer_reaches_the_tool_call() {
    let (response, observed) = with_client(PermissionAnswer::Select("allow"), async |cx, _| {
        initialize(&cx).await?;
        let session = new_session(&cx).await?;
        prompt_and_wait(&cx, &session.session_id, "needs permission").await
    })
    .await;
    assert_eq!(response.stop_reason, StopReason::EndTurn);
    assert_eq!(
        *observed.permission_requests.lock().expect("lock"),
        vec!["Bash".to_string()]
    );
    assert!(observed.saw(|u| matches!(u, SessionUpdate::ToolCall(call) if call.title == "Bash")));
    assert!(observed.saw(|u| matches!(
        u,
        SessionUpdate::ToolCallUpdate(update) if update.fields.status == Some(ToolCallStatus::Completed)
    )));
    assert!(observed.messages().ends_with("permission: allowed"));
}

#[tokio::test]
async fn a_rejected_permission_fails_the_tool_call() {
    let (_, observed) = with_client(PermissionAnswer::Select("reject"), async |cx, _| {
        initialize(&cx).await?;
        let session = new_session(&cx).await?;
        prompt_and_wait(&cx, &session.session_id, "needs permission").await
    })
    .await;
    assert!(observed.saw(|u| matches!(
        u,
        SessionUpdate::ToolCallUpdate(update) if update.fields.status == Some(ToolCallStatus::Failed)
    )));
    assert!(observed.messages().ends_with("permission: rejected"));
}

#[tokio::test]
async fn cancel_mid_turn_ends_the_turn_cancelled_and_the_next_prompt_runs_fresh() {
    let (responses, observed) = with_client(PermissionAnswer::Cancel, async |cx, observed| {
        initialize(&cx).await?;
        let session = new_session(&cx).await?;
        let sid = session.session_id.clone();
        let hanging = cx.send_request(prompt(&sid, "hang on"));
        observed
            .wait_for(|u| {
                matches!(u, SessionUpdate::AgentMessageChunk(c)
                    if serde_json::to_string(&c.content).expect("json").contains("hanging"))
            })
            .await;
        cx.send_notification(CancelNotification::new(sid.clone()))?;
        let cancelled = hanging.block_task().await?;
        let next = prompt_and_wait(&cx, &sid, "after the cancel").await?;
        Ok((cancelled.stop_reason, next.stop_reason))
    })
    .await;
    assert_eq!(responses, (StopReason::Cancelled, StopReason::EndTurn));
    assert!(observed.messages().contains("echo: after the cancel"));
}

#[tokio::test]
async fn cancel_while_a_permission_is_open_answers_the_turn_cancelled_without_the_client() {
    // The client HOLDS the permission request and never answers it: the
    // agent's own cancellation must unblock the provider.
    let (response, observed) = with_client(PermissionAnswer::Hold, async |cx, observed| {
        initialize(&cx).await?;
        let session = new_session(&cx).await?;
        let sid = session.session_id.clone();
        let asking = cx.send_request(prompt(&sid, "needs permission"));
        tokio::time::timeout(fake_client::WAIT, async {
            while observed.pending_permission.lock().expect("lock").is_none() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("the permission request was held");
        cx.send_notification(CancelNotification::new(sid.clone()))?;
        let response = asking.block_task().await?;
        // Now answer it the way a conforming client would have.
        observed.answer_pending(RequestPermissionOutcome::Cancelled);
        Ok(response)
    })
    .await;
    assert_eq!(response.stop_reason, StopReason::Cancelled);
    assert!(observed.messages().ends_with("permission: cancelled"));
}

#[tokio::test]
async fn a_turn_queued_behind_a_cancelled_one_is_cancelled_too() {
    let (stops, _) = with_client(PermissionAnswer::Cancel, async |cx, observed| {
        initialize(&cx).await?;
        let session = new_session(&cx).await?;
        let sid = session.session_id.clone();
        let first = cx.send_request(prompt(&sid, "hang one"));
        observed
            .wait_for(|u| {
                matches!(u, SessionUpdate::AgentMessageChunk(c)
                    if serde_json::to_string(&c.content).expect("json").contains("hanging"))
            })
            .await;
        // Queued behind the hanging turn, before the cancel.
        let second = cx.send_request(prompt(&sid, "queued"));
        cx.send_notification(CancelNotification::new(sid.clone()))?;
        let first = first.block_task().await?;
        let second = second.block_task().await?;
        // Sent after the cancel: runs on the fresh cancellation.
        let third = prompt_and_wait(&cx, &sid, "third").await?;
        Ok((first.stop_reason, second.stop_reason, third.stop_reason))
    })
    .await;
    assert_eq!(
        stops,
        (
            StopReason::Cancelled,
            StopReason::Cancelled,
            StopReason::EndTurn
        )
    );
}

#[tokio::test]
async fn session_load_replays_the_history_before_answering() {
    let sid = SessionId::new("sess-from-before");
    let (response, observed) = with_client(PermissionAnswer::Cancel, {
        let sid = sid.clone();
        async move |cx, observed| {
            initialize(&cx).await?;
            let loaded = cx
                .send_request(LoadSessionRequest::new(sid.clone(), std::env::temp_dir()))
                .block_task()
                .await?;
            assert!(
                observed.messages().contains("resumed sess-from-before"),
                "the replay arrived before the response"
            );
            assert!(loaded.config_options.is_some());
            prompt_and_wait(&cx, &sid, "continue").await
        }
    })
    .await;
    assert_eq!(response.stop_reason, StopReason::EndTurn);
    let sessions: Vec<String> = observed
        .updates
        .lock()
        .expect("lock")
        .iter()
        .map(|(s, _)| s.to_string())
        .collect();
    assert!(sessions.iter().all(|s| s == "sess-from-before"));
}

#[tokio::test]
async fn loading_an_open_session_is_refused() {
    let (err, _) = with_client(PermissionAnswer::Cancel, async |cx, _| {
        initialize(&cx).await?;
        let session = new_session(&cx).await?;
        Ok(cx
            .send_request(LoadSessionRequest::new(
                session.session_id.clone(),
                std::env::temp_dir(),
            ))
            .block_task()
            .await
            .expect_err("already open"))
    })
    .await;
    assert!(
        err.data
            .as_ref()
            .and_then(|d| d.as_str())
            .is_some_and(|d| d.starts_with("session already open")),
        "{err:?}"
    );
}

#[tokio::test]
async fn a_prompt_on_an_unknown_session_is_refused() {
    let (err, _) = with_client(PermissionAnswer::Cancel, async |cx, _| {
        initialize(&cx).await?;
        Ok(cx
            .send_request(prompt(&SessionId::new("nope"), "hi"))
            .block_task()
            .await
            .expect_err("unknown session"))
    })
    .await;
    assert_eq!(err.data, Some(serde_json::json!("unknown session: nope")));
}

#[tokio::test]
async fn a_provider_failure_is_an_internal_error_not_a_stop_reason() {
    let (err, _) = with_client(PermissionAnswer::Cancel, async |cx, _| {
        initialize(&cx).await?;
        let session = new_session(&cx).await?;
        Ok(prompt_and_wait(&cx, &session.session_id, "please fail")
            .await
            .expect_err("the scripted failure"))
    })
    .await;
    assert_eq!(err.data, Some(serde_json::json!("scripted failure")));
}

#[tokio::test]
async fn the_model_switch_takes_effect_on_the_next_turn_and_refuses_strangers() {
    let (result, observed) = with_client(PermissionAnswer::Cancel, async |cx, _| {
        initialize(&cx).await?;
        let session = new_session(&cx).await?;
        let sid = session.session_id.clone();
        let switched = cx
            .send_request(SetSessionConfigOptionRequest::new(
                sid.clone(),
                "model",
                SessionConfigOptionValue::ValueId {
                    value: ScriptedProvider::SLOW.into(),
                },
            ))
            .block_task()
            .await?;
        let picker = serde_json::to_value(&switched.config_options).expect("json");
        assert_eq!(picker[0]["currentValue"], ScriptedProvider::SLOW);
        let stranger = cx
            .send_request(SetSessionConfigOptionRequest::new(
                sid.clone(),
                "model",
                SessionConfigOptionValue::ValueId {
                    value: "gpt-9".into(),
                },
            ))
            .block_task()
            .await
            .expect_err("not a model the provider offers");
        assert_eq!(
            stranger.data,
            Some(serde_json::json!("unknown model: gpt-9"))
        );
        prompt_and_wait(&cx, &sid, "which model").await
    })
    .await;
    assert_eq!(result.stop_reason, StopReason::EndTurn);
    assert_eq!(
        observed.messages(),
        format!("[{}] echo: which model", ScriptedProvider::SLOW)
    );
}
