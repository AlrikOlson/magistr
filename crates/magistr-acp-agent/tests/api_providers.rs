//! The API providers end to end — through the ACP agent and the scripted
//! client — against a fake HTTP server replaying each API's streaming wire
//! from a fixture. Hermetic: no key, no network beyond loopback.

mod fake_client;
mod fake_http;
#[path = "../../magistr-cli/tests/support/in_env.rs"]
mod in_env;

use agent_client_protocol::schema::v1::{CancelNotification, SessionUpdate, StopReason};
use fake_client::{
    PermissionAnswer, initialize, new_session, prompt, prompt_and_wait, with_provider,
};
use fake_http::{FakeHttp, Reply};
use magistr_acp_agent::{ApiProvider, Gemini, Ollama, OpenAi, Provider};

fn fixture(path: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/{path}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("the fixture exists")
}

fn cost_of(update: &SessionUpdate) -> Option<f64> {
    match update {
        SessionUpdate::UsageUpdate(usage) => usage.cost.as_ref().map(|c| c.amount),
        _ => None,
    }
}

#[tokio::test]
async fn openai_rejects_errors_and_invalid_terminal_sequences() {
    let finish = "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n";
    for (body, expected) in [
        (
            "data: {\"error\":{\"message\":\"overloaded\"}}\n\n".to_owned(),
            "overloaded",
        ),
        (
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n".to_owned(),
            "unsupported finish",
        ),
        (
            format!("{finish}{finish}data: [DONE]\n\n"),
            "duplicate provider finish",
        ),
        (
            format!("{finish}data: {{\"choices\":[{{\"delta\":{{\"content\":\"late\"}}}}]}}\n\n"),
            "text after provider finish",
        ),
        (
            format!("{finish}data: [DONE]\n\ndata: [DONE]\n\n"),
            "event after end sentinel",
        ),
    ] {
        let server = FakeHttp::serve(vec![Reply::stream("text/event-stream", &body)]).await;
        assert_stream_error(OpenAi::new("test", &server.url), expected).await;
    }
}

async fn assert_stream_error<A: magistr_acp_agent::ChatApi>(api: A, expected: &str) {
    let (_, observed) = with_provider(
        ApiProvider::new(api),
        PermissionAnswer::Cancel,
        async |cx, _| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            let err = prompt_and_wait(&cx, &session.session_id, "hi")
                .await
                .expect_err("invalid stream");
            assert!(
                err.data
                    .as_ref()
                    .and_then(|data| data.as_str())
                    .is_some_and(|data| data.contains(expected)),
                "{err:?}"
            );
            Ok(())
        },
    )
    .await;
    assert!(!observed.saw(|u| matches!(u, SessionUpdate::UsageUpdate(_))));
}

#[tokio::test]
async fn every_api_rejects_truncated_transport_and_malformed_tail() {
    for name in ["openai", "gemini", "ollama"] {
        let (content_type, complete) = match name {
            "openai" => ("text/event-stream", fixture("openai/turn.sse")),
            "gemini" => ("text/event-stream", fixture("gemini/turn.sse")),
            _ => ("application/x-ndjson", fixture("ollama/turn.ndjson")),
        };
        for truncated in [true, false] {
            let reply = if truncated {
                Reply::Truncated {
                    content_type,
                    body: complete.clone(),
                }
            } else {
                let mut malformed = complete.trim_end().to_owned();
                malformed.pop();
                Reply::stream(content_type, &malformed)
            };
            let mut replies = Vec::new();
            if name == "ollama" {
                replies.push(Reply::ok("application/json", fixture("ollama/tags.json")));
            }
            replies.push(reply);
            let server = FakeHttp::serve(replies).await;
            let expected = if truncated {
                "stream failed"
            } else {
                "cannot parse"
            };
            match name {
                "openai" => assert_stream_error(OpenAi::new("test", &server.url), expected).await,
                "gemini" => assert_stream_error(Gemini::new("test", &server.url), expected).await,
                _ => {
                    assert_stream_error(
                        Ollama::discover_at(&server.url).await.expect("tags"),
                        expected,
                    )
                    .await
                }
            }
        }
    }
}

#[tokio::test]
async fn failed_streams_settle_history_before_the_next_prompt() {
    for first in [
        "",
        "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\ndata: {",
    ] {
        let server = FakeHttp::serve(vec![
            Reply::stream("text/event-stream", first),
            Reply::stream("text/event-stream", &fixture("openai/turn.sse")),
        ])
        .await;
        let provider = ApiProvider::new(OpenAi::new("test", &server.url));
        with_provider(provider, PermissionAnswer::Cancel, async |cx, _| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            assert!(
                prompt_and_wait(&cx, &session.session_id, "first")
                    .await
                    .is_err()
            );
            assert_eq!(
                prompt_and_wait(&cx, &session.session_id, "retry")
                    .await?
                    .stop_reason,
                StopReason::EndTurn
            );
            Ok(())
        })
        .await;
        let requests = server.recorded();
        let second = requests[1].json();
        let messages = second["messages"].as_array().expect("messages");
        if first.is_empty() {
            assert_eq!(messages.len(), 1);
        } else {
            assert_eq!(messages.len(), 3);
            assert_eq!(messages[1]["role"], "assistant");
            assert_eq!(messages[1]["content"], "partial");
        }
        assert_eq!(messages.last().expect("retry")["content"], "retry");
    }
}

#[tokio::test]
async fn openai_success_without_usage_accepts_an_unterminated_sentinel() {
    let server = FakeHttp::serve(vec![Reply::stream("text/event-stream",
        "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"},\"finish_reason\":\"stop\"}]}\r\n\r\ndata: [DONE]"
    )]).await;
    let provider = ApiProvider::new(OpenAi::new("test", &server.url));
    let (response, observed) = with_provider(provider, PermissionAnswer::Cancel, async |cx, _| {
        initialize(&cx).await?;
        let session = new_session(&cx).await?;
        prompt_and_wait(&cx, &session.session_id, "hi").await
    })
    .await;
    assert_eq!(response.stop_reason, StopReason::EndTurn);
    assert_eq!(observed.messages(), "ok");
    assert!(!observed.saw(|u| matches!(u, SessionUpdate::UsageUpdate(_))));
}

// ── OpenAI ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn openai_streams_the_reply_and_prices_the_usage_from_the_table() {
    let server = FakeHttp::serve(vec![Reply::stream(
        "text/event-stream",
        &fixture("openai/turn.sse"),
    )])
    .await;
    let provider = ApiProvider::new(OpenAi::new("sk-test", &server.url));
    assert_eq!(provider.name(), "openai");
    assert_eq!(provider.models()[0].id, "gpt-4o");

    let (response, observed) = with_provider(provider, PermissionAnswer::Cancel, async |cx, _| {
        let init = initialize(&cx).await?;
        assert_eq!(init.meta.expect("_meta")["magistr"]["provider"], "openai");
        let session = new_session(&cx).await?;
        prompt_and_wait(&cx, &session.session_id, "why is the sky blue").await
    })
    .await;
    assert_eq!(response.stop_reason, StopReason::EndTurn);
    assert_eq!(observed.messages(), "Rayleigh scattering.");
    // 1M prompt tokens at $2.50 + 100k completion tokens at $10.00 (gpt-4o).
    assert!(observed.saw(|u| matches!(
        u,
        SessionUpdate::UsageUpdate(usage)
            if usage.used == 1_100_000 && usage.size == 128_000
    )));
    let cost = observed
        .updates
        .lock()
        .expect("lock")
        .iter()
        .find_map(|(_, u)| cost_of(u))
        .expect("a cost on the usage update");
    assert!((cost - 3.50).abs() < 1e-9, "{cost}");
    assert!(
        !observed.saw(|u| matches!(u, SessionUpdate::ToolCall(_))),
        "no tool, no permission"
    );
    assert!(
        observed
            .permission_requests
            .lock()
            .expect("lock")
            .is_empty()
    );

    let requests = server.recorded();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].target, "/chat/completions");
    assert_eq!(requests[0].header("authorization"), Some("Bearer sk-test"));
    let body = requests[0].json();
    assert_eq!(body["model"], "gpt-4o");
    assert_eq!(body["stream_options"]["include_usage"], true);
    assert_eq!(body["messages"][0]["content"], "why is the sky blue");
}

#[tokio::test]
async fn openai_carries_the_history_into_the_second_turn_and_the_cost_accumulates() {
    let server = FakeHttp::serve(vec![
        Reply::stream("text/event-stream", &fixture("openai/turn.sse")),
        Reply::stream("text/event-stream", &fixture("openai/turn_second.sse")),
    ])
    .await;
    let provider = ApiProvider::new(OpenAi::new("sk-test", &server.url));
    let (_, observed) = with_provider(provider, PermissionAnswer::Cancel, async |cx, _| {
        initialize(&cx).await?;
        let session = new_session(&cx).await?;
        prompt_and_wait(&cx, &session.session_id, "why is the sky blue").await?;
        prompt_and_wait(&cx, &session.session_id, "shorter").await
    })
    .await;
    assert_eq!(observed.messages(), "Rayleigh scattering.Blue light.");
    let requests = server.recorded();
    let second = requests[1].json();
    let roles: Vec<&str> = second["messages"]
        .as_array()
        .expect("messages")
        .iter()
        .filter_map(|m| m["role"].as_str())
        .collect();
    assert_eq!(roles, ["user", "assistant", "user"]);
    assert_eq!(second["messages"][1]["content"], "Rayleigh scattering.");
    assert_eq!(second["messages"][2]["content"], "shorter");
    let costs: Vec<f64> = observed
        .updates
        .lock()
        .expect("lock")
        .iter()
        .filter_map(|(_, u)| cost_of(u))
        .collect();
    assert_eq!(costs.len(), 2);
    assert!(costs[1] > costs[0], "cumulative: {costs:?}");
}

#[tokio::test]
async fn openai_cancel_mid_stream_drops_the_connection_and_ends_the_turn_cancelled() {
    let server = FakeHttp::serve(vec![Reply::held(
        "text/event-stream",
        &fixture("openai/turn_partial.sse"),
    )])
    .await;
    let provider = ApiProvider::new(OpenAi::new("sk-test", &server.url));
    let (stop, observed) =
        with_provider(provider, PermissionAnswer::Cancel, async |cx, observed| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            let sid = session.session_id.clone();
            let counting = cx.send_request(prompt(&sid, "count to 400"));
            observed
                .wait_for(|u| matches!(u, SessionUpdate::AgentMessageChunk(_)))
                .await;
            cx.send_notification(CancelNotification::new(sid.clone()))?;
            Ok(counting.block_task().await?.stop_reason)
        })
        .await;
    assert_eq!(stop, StopReason::Cancelled);
    assert_eq!(observed.messages(), "one, two,");
    server.wait_for_close(1).await;
    assert!(
        !observed.saw(|u| matches!(u, SessionUpdate::UsageUpdate(_))),
        "a cancelled turn reports no usage"
    );
}

#[tokio::test]
async fn openai_refusal_fails_the_turn_with_the_status_and_the_body() {
    let server = FakeHttp::serve(vec![Reply::error(401, fixture("openai/error_401.json"))]).await;
    let provider = ApiProvider::new(OpenAi::new("sk-test", &server.url));
    let (err, _) = with_provider(provider, PermissionAnswer::Cancel, async |cx, _| {
        initialize(&cx).await?;
        let session = new_session(&cx).await?;
        Ok(prompt_and_wait(&cx, &session.session_id, "hi")
            .await
            .expect_err("the API refused"))
    })
    .await;
    let data = err
        .data
        .and_then(|d| d.as_str().map(str::to_owned))
        .expect("data");
    assert!(data.contains("openai API error 401"), "{data}");
    assert!(data.contains("invalid_api_key"), "{data}");
}

#[test]
fn openai_without_a_key_is_refused_naming_the_variable() {
    // Exercise the real constructor in a child with its key removed, even
    // when the developer running this suite has credentials configured.
    if !in_env::in_env(&[(OpenAi::KEY_ENV, None)]) {
        return;
    }
    let err = OpenAi::from_env().expect_err("no key");
    assert_eq!(
        err.to_string(),
        "provider error: OPENAI_API_KEY environment variable not set"
    );
}

#[tokio::test]
async fn openai_requires_both_finish_and_sentinel() {
    for body in [
        "",
        "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n",
        "data: [DONE]\n\n",
        "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
    ] {
        let server = FakeHttp::serve(vec![Reply::stream("text/event-stream", body)]).await;
        let provider = ApiProvider::new(OpenAi::new("sk-test", &server.url));
        let (_, observed) = with_provider(provider, PermissionAnswer::Cancel, async |cx, _| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            assert!(
                prompt_and_wait(&cx, &session.session_id, "hi")
                    .await
                    .is_err(),
                "incomplete stream accepted: {body:?}"
            );
            Ok(())
        })
        .await;
        assert!(!observed.saw(|u| matches!(u, SessionUpdate::UsageUpdate(_))));
    }
}

#[tokio::test]
async fn openai_length_is_max_tokens_without_fabricated_usage() {
    let server = FakeHttp::serve(vec![Reply::stream("text/event-stream",
        "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"},\"finish_reason\":\"length\"}]}\n\ndata: [DONE]\n\n"
    )]).await;
    let provider = ApiProvider::new(OpenAi::new("sk-test", &server.url));
    let (response, observed) = with_provider(provider, PermissionAnswer::Cancel, async |cx, _| {
        initialize(&cx).await?;
        let session = new_session(&cx).await?;
        prompt_and_wait(&cx, &session.session_id, "hi").await
    })
    .await;
    assert_eq!(response.stop_reason, StopReason::MaxTokens);
    assert_eq!(observed.messages(), "partial");
    assert!(!observed.saw(|u| matches!(u, SessionUpdate::UsageUpdate(_))));
}

#[tokio::test]
async fn openai_refusal_deltas_and_content_filter_are_refusals() {
    for body in [
        "data: {\"choices\":[{\"delta\":{\"refusal\":\"Cannot help.\"}}]}\n\ndata: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n",
        "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"content_filter\"}]}\n\ndata: [DONE]\n\n",
    ] {
        let server = FakeHttp::serve(vec![Reply::stream("text/event-stream", body)]).await;
        let provider = ApiProvider::new(OpenAi::new("sk-test", &server.url));
        let (response, _) = with_provider(provider, PermissionAnswer::Cancel, async |cx, _| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            prompt_and_wait(&cx, &session.session_id, "hi").await
        })
        .await;
        assert_eq!(response.stop_reason, StopReason::Refusal);
    }
}

#[tokio::test]
async fn gemini_terminal_reasons_survive_http() {
    for (body, expected) in [
        (
            r#"{"candidates":[{"finishReason":"STOP"}]}"#,
            Some(StopReason::EndTurn),
        ),
        (
            r#"{"candidates":[{"finishReason":"MAX_TOKENS"}]}"#,
            Some(StopReason::MaxTokens),
        ),
        (
            r#"{"candidates":[{"finishReason":"SAFETY"}]}"#,
            Some(StopReason::Refusal),
        ),
        (
            r#"{"promptFeedback":{"blockReason":"SAFETY"}}"#,
            Some(StopReason::Refusal),
        ),
        (
            r#"{"candidates":[{"content":{"parts":[{"text":"partial"}]}}]}"#,
            None,
        ),
        (r#"{"usageMetadata":{"promptTokenCount":1}}"#, None),
        (
            r#"{"candidates":[{"finishReason":"MALFORMED_FUNCTION_CALL"}]}"#,
            None,
        ),
        (r#"{"error":{"message":"provider failed"}}"#, None),
    ] {
        let server = FakeHttp::serve(vec![Reply::stream(
            "text/event-stream",
            &format!("data: {body}\n\n"),
        )])
        .await;
        let provider = ApiProvider::new(Gemini::new("test", &server.url));
        let (_, observed) = with_provider(provider, PermissionAnswer::Cancel, async |cx, _| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            let result = prompt_and_wait(&cx, &session.session_id, "hi").await;
            match expected {
                Some(stop) => assert_eq!(result?.stop_reason, stop, "{body}"),
                None => assert!(result.is_err(), "{body}"),
            }
            Ok(())
        })
        .await;
        assert!(!observed.saw(|u| matches!(u, SessionUpdate::UsageUpdate(_))));
    }
}

#[tokio::test]
async fn ollama_terminal_reasons_survive_http() {
    for (body, expected) in [
        (
            r#"{"done":true,"done_reason":"stop"}"#,
            Some(StopReason::EndTurn),
        ),
        (r#"{"done":true}"#, Some(StopReason::EndTurn)),
        (
            r#"{"done":true,"done_reason":"length"}"#,
            Some(StopReason::MaxTokens),
        ),
        (r#"{"done":false,"message":{"content":"partial"}}"#, None),
        (r#"{"error":"provider failed"}"#, None),
        (r#"{"done":true,"done_reason":"unload"}"#, None),
        (r#"{"done":true,"done_reason":"future"}"#, None),
    ] {
        let server = FakeHttp::serve(vec![
            Reply::ok("application/json", fixture("ollama/tags.json")),
            Reply::stream("application/x-ndjson", &format!("{body}\n")),
        ])
        .await;
        let provider = ApiProvider::new(Ollama::discover_at(&server.url).await.expect("tags"));
        let (_, observed) = with_provider(provider, PermissionAnswer::Cancel, async |cx, _| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            let result = prompt_and_wait(&cx, &session.session_id, "hi").await;
            match expected {
                Some(stop) => assert_eq!(result?.stop_reason, stop, "{body}"),
                None => assert!(result.is_err(), "{body}"),
            }
            Ok(())
        })
        .await;
        assert!(!observed.saw(|u| matches!(u, SessionUpdate::UsageUpdate(_))));
    }
}

// ── Gemini ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn gemini_streams_the_parts_and_prices_the_last_usage_at_the_flash_rate() {
    let server = FakeHttp::serve(vec![Reply::stream(
        "text/event-stream",
        &fixture("gemini/turn.sse"),
    )])
    .await;
    let provider = ApiProvider::new(Gemini::new("AIza-test", &server.url));
    let (response, observed) = with_provider(provider, PermissionAnswer::Cancel, async |cx, _| {
        initialize(&cx).await?;
        let session = new_session(&cx).await?;
        prompt_and_wait(&cx, &session.session_id, "why is the sky blue").await
    })
    .await;
    assert_eq!(response.stop_reason, StopReason::EndTurn);
    assert_eq!(observed.messages(), "Rayleigh scattering.");
    assert!(observed.saw(|u| matches!(
        u,
        SessionUpdate::UsageUpdate(usage) if usage.used == 12 && usage.size == 1_048_576
    )));
    let cost = observed
        .updates
        .lock()
        .expect("lock")
        .iter()
        .find_map(|(_, u)| cost_of(u))
        .expect("a cost");
    // 8 × 0.075 + 4 × 0.30 per million.
    assert!(
        (cost - (8.0 * 0.075 + 4.0 * 0.30) / 1e6).abs() < 1e-15,
        "{cost}"
    );

    let requests = server.recorded();
    assert_eq!(
        requests[0].target,
        "/models/gemini-2.5-flash:streamGenerateContent?alt=sse"
    );
    assert_eq!(requests[0].header("x-goog-api-key"), Some("AIza-test"));
    let body = requests[0].json();
    assert_eq!(body["contents"][0]["role"], "user");
    assert_eq!(
        body["contents"][0]["parts"][0]["text"],
        "why is the sky blue"
    );
}

#[tokio::test]
async fn gemini_cancel_mid_stream_keeps_the_partial_reply_as_the_model_turn() {
    let server = FakeHttp::serve(vec![
        Reply::held("text/event-stream", &fixture("gemini/turn_partial.sse")),
        Reply::stream("text/event-stream", &fixture("gemini/turn.sse")),
    ])
    .await;
    let provider = ApiProvider::new(Gemini::new("AIza-test", &server.url));
    let (stops, _) = with_provider(provider, PermissionAnswer::Cancel, async |cx, observed| {
        initialize(&cx).await?;
        let session = new_session(&cx).await?;
        let sid = session.session_id.clone();
        let counting = cx.send_request(prompt(&sid, "count to 400"));
        observed
            .wait_for(|u| matches!(u, SessionUpdate::AgentMessageChunk(_)))
            .await;
        cx.send_notification(CancelNotification::new(sid.clone()))?;
        let cancelled = counting.block_task().await?.stop_reason;
        let next = prompt_and_wait(&cx, &sid, "never mind").await?.stop_reason;
        Ok((cancelled, next))
    })
    .await;
    assert_eq!(stops, (StopReason::Cancelled, StopReason::EndTurn));
    server.wait_for_close(1).await;
    let second = server.recorded()[1].json();
    let roles: Vec<&str> = second["contents"]
        .as_array()
        .expect("contents")
        .iter()
        .filter_map(|c| c["role"].as_str())
        .collect();
    assert_eq!(roles, ["user", "model", "user"], "still alternating");
    assert_eq!(second["contents"][1]["parts"][0]["text"], "one, two,");
}

#[tokio::test]
async fn gemini_refusal_fails_the_turn_with_the_status_and_the_body() {
    let server = FakeHttp::serve(vec![Reply::error(400, fixture("gemini/error_400.json"))]).await;
    let provider = ApiProvider::new(Gemini::new("AIza-test", &server.url));
    let (err, _) = with_provider(provider, PermissionAnswer::Cancel, async |cx, _| {
        initialize(&cx).await?;
        let session = new_session(&cx).await?;
        Ok(prompt_and_wait(&cx, &session.session_id, "hi")
            .await
            .expect_err("the API refused"))
    })
    .await;
    let data = err
        .data
        .and_then(|d| d.as_str().map(str::to_owned))
        .expect("data");
    assert!(data.contains("gemini API error 400"), "{data}");
    assert!(data.contains("API_KEY_INVALID"), "{data}");
}

#[test]
fn gemini_without_a_key_is_refused_naming_both_variables() {
    if !in_env::in_env(&[(Gemini::KEY_ENVS[0], None), (Gemini::KEY_ENVS[1], None)]) {
        return;
    }
    let err = Gemini::from_env().expect_err("no key");
    assert_eq!(
        err.to_string(),
        "provider error: GEMINI_API_KEY or GOOGLE_API_KEY environment variable not set"
    );
}

// ── Ollama ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn ollama_lists_the_daemons_models_streams_the_lines_and_costs_nothing() {
    let server = FakeHttp::serve(vec![
        Reply::ok("application/json", fixture("ollama/tags.json")),
        Reply::stream("application/x-ndjson", &fixture("ollama/turn.ndjson")),
    ])
    .await;
    let ollama = Ollama::discover_at(&server.url)
        .await
        .expect("the daemon answered /api/tags");
    let provider = ApiProvider::new(ollama);
    let ids: Vec<String> = provider.models().into_iter().map(|m| m.id).collect();
    assert_eq!(ids, ["llama3.1:latest", "qwen2.5-coder:7b"]);

    let (response, observed) = with_provider(provider, PermissionAnswer::Cancel, async |cx, _| {
        initialize(&cx).await?;
        let session = new_session(&cx).await?;
        prompt_and_wait(&cx, &session.session_id, "why is the sky blue").await
    })
    .await;
    assert_eq!(response.stop_reason, StopReason::EndTurn);
    assert_eq!(observed.messages(), "Rayleigh scattering.");
    assert!(observed.saw(|u| matches!(
        u,
        SessionUpdate::UsageUpdate(usage) if usage.used == 13 && usage.size == Ollama::WINDOW
    )));
    let cost = observed
        .updates
        .lock()
        .expect("lock")
        .iter()
        .find_map(|(_, u)| cost_of(u))
        .expect("a cost");
    assert_eq!(cost, 0.0);

    let requests = server.recorded();
    assert_eq!(requests[0].target, "/api/tags");
    assert_eq!(requests[1].target, "/api/chat");
    let body = requests[1].json();
    assert_eq!(body["model"], "llama3.1:latest");
    assert_eq!(body["stream"], true);
}

#[tokio::test]
async fn ollama_cancel_mid_stream_drops_the_connection() {
    let server = FakeHttp::serve(vec![
        Reply::ok("application/json", fixture("ollama/tags.json")),
        Reply::held(
            "application/x-ndjson",
            &fixture("ollama/turn_partial.ndjson"),
        ),
    ])
    .await;
    let ollama = Ollama::discover_at(&server.url).await.expect("tags");
    let (stop, observed) = with_provider(
        ApiProvider::new(ollama),
        PermissionAnswer::Cancel,
        async |cx, observed| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            let sid = session.session_id.clone();
            let counting = cx.send_request(prompt(&sid, "count to 400"));
            observed
                .wait_for(|u| matches!(u, SessionUpdate::AgentMessageChunk(_)))
                .await;
            cx.send_notification(CancelNotification::new(sid.clone()))?;
            Ok(counting.block_task().await?.stop_reason)
        },
    )
    .await;
    assert_eq!(stop, StopReason::Cancelled);
    assert_eq!(observed.messages(), "one, two,");
    server.wait_for_close(2).await;
}

#[tokio::test]
async fn ollama_with_nothing_pulled_is_refused_with_the_remedy() {
    let server = FakeHttp::serve(vec![Reply::ok(
        "application/json",
        fixture("ollama/tags_empty.json"),
    )])
    .await;
    let err = Ollama::discover_at(&server.url)
        .await
        .expect_err("no models");
    assert!(err.to_string().contains("ollama pull"), "{err}");
}
