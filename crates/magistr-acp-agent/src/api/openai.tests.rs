use super::*;

#[test]
fn terminal_and_refusal_evidence_is_independent_of_usage() {
    for (reason, stop) in [
        ("stop", StopReason::EndTurn),
        ("length", StopReason::MaxTokens),
        ("content_filter", StopReason::Refusal),
    ] {
        let line = format!(r#"data: {{"choices":[{{"delta":{{}},"finish_reason":"{reason}"}}]}}"#);
        assert_eq!(
            api().decode(&line).expect("terminal"),
            vec![StreamEvent::Finish(stop)]
        );
    }
    assert_eq!(
        api().decode("data: [DONE]").expect("sentinel"),
        vec![StreamEvent::Done]
    );
    assert_eq!(
        api()
            .decode(r#"data: {"choices":[{"delta":{"refusal":"No."}}]}"#)
            .expect("refusal"),
        vec![StreamEvent::Refusal("No.".to_owned())]
    );
}

#[test]
fn unsupported_finishes_and_stream_errors_fail() {
    for reason in ["tool_calls", "function_call", "future"] {
        let line = format!(r#"data: {{"choices":[{{"delta":{{}},"finish_reason":"{reason}"}}]}}"#);
        assert!(
            api()
                .decode(&line)
                .expect_err("unsupported")
                .to_string()
                .contains(reason)
        );
    }
    assert!(
        api()
            .decode(r#"data: {"error":{"message":"overloaded"}}"#)
            .expect_err("error")
            .to_string()
            .contains("overloaded")
    );
}

fn api() -> OpenAi {
    OpenAi::new("sk-test", "https://example.test/v1/")
}

#[test]
fn the_registry_starts_on_gpt_4o_at_the_plugins_prices() {
    let models = api().models().to_vec();
    assert_eq!(models[0].id, "gpt-4o");
    assert_eq!(models[0].pricing, Pricing::per_million(2.50, 10.00));
    assert!(models.iter().any(|m| m.id == "o3-mini"));
}

#[test]
fn a_text_delta_decodes_to_text_and_the_usage_chunk_to_usage() {
    let api = api();
    let text = api
        .decode(r#"data: {"choices":[{"delta":{"content":"Hel"},"finish_reason":null}]}"#)
        .expect("decodes");
    assert_eq!(text, vec![StreamEvent::Text("Hel".to_owned())]);
    let usage = api
        .decode(
            r#"data: {"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":3,"total_tokens":15}}"#,
        )
        .expect("decodes");
    assert_eq!(
        usage,
        vec![StreamEvent::Usage {
            input: 12,
            output: 3
        }]
    );
}

#[test]
fn blank_and_comment_lines_carry_nothing() {
    let api = api();
    for line in ["", ": keep-alive", "event: message"] {
        assert!(api.decode(line).expect("decodes").is_empty(), "{line:?}");
    }
}

#[test]
fn a_malformed_chunk_is_a_provider_error_naming_the_data() {
    let err = api().decode("data: {not json").expect_err("malformed");
    assert!(err.to_string().contains("{not json"), "{err}");
}

#[tokio::test]
async fn the_request_carries_the_history_the_stream_flags_and_the_bearer_key() {
    let api = api();
    let history = vec![
        ChatMessage::new(Role::User, "hi"),
        ChatMessage::new(Role::Assistant, "hello"),
        ChatMessage::new(Role::User, "more"),
    ];
    let request = api
        .request(&reqwest::Client::new(), "gpt-4o-mini", &history)
        .build()
        .expect("builds");
    assert_eq!(
        request.url().as_str(),
        "https://example.test/v1/chat/completions",
        "the trailing slash of the base URL is not doubled"
    );
    assert_eq!(
        request.headers()["authorization"].to_str().expect("ascii"),
        "Bearer sk-test"
    );
    let body: serde_json::Value =
        serde_json::from_slice(request.body().and_then(|b| b.as_bytes()).expect("a body"))
            .expect("json");
    assert_eq!(body["model"], "gpt-4o-mini");
    assert_eq!(body["stream"], true);
    assert_eq!(body["stream_options"]["include_usage"], true);
    assert_eq!(body["messages"][1]["role"], "assistant");
    assert_eq!(body["messages"][2]["content"], "more");
}
