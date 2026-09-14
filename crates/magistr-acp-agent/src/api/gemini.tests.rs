use super::*;

#[test]
fn terminal_and_block_evidence_is_independent_of_usage() {
    for (reason, stop) in [
        ("STOP", StopReason::EndTurn),
        ("MAX_TOKENS", StopReason::MaxTokens),
        ("SAFETY", StopReason::Refusal),
        ("RECITATION", StopReason::Refusal),
        ("SPII", StopReason::Refusal),
    ] {
        let line = format!(r#"data: {{"candidates":[{{"finishReason":"{reason}"}}]}}"#);
        assert_eq!(
            api().decode(&line).expect("terminal"),
            vec![StreamEvent::Finish(stop)]
        );
    }
    assert_eq!(
        api()
            .decode(r#"data: {"promptFeedback":{"blockReason":"SAFETY"}}"#)
            .expect("block"),
        vec![StreamEvent::Finish(StopReason::Refusal)]
    );
}

#[test]
fn malformed_unknown_and_error_outcomes_fail() {
    for reason in [
        "OTHER",
        "FINISH_REASON_UNSPECIFIED",
        "MALFORMED_FUNCTION_CALL",
        "future",
    ] {
        let line = format!(r#"data: {{"candidates":[{{"finishReason":"{reason}"}}]}}"#);
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
    assert!(api().decode("data: {").is_err());
}

fn api() -> Gemini {
    Gemini::new("AIza-test", "https://example.test/v1beta")
}

#[test]
fn the_registry_starts_on_flash_at_the_backends_prices() {
    let models = api().models().to_vec();
    assert_eq!(models[0].id, "gemini-2.5-flash");
    assert_eq!(models[0].pricing, Pricing::per_million(0.075, 0.30));
    let pro = models
        .iter()
        .find(|m| m.id == "gemini-2.5-pro")
        .expect("pro");
    assert_eq!(pro.pricing, Pricing::per_million(1.25, 5.00));
    assert_eq!(pro.context_window, 1_048_576);
}

#[test]
fn a_chunk_decodes_to_its_text_parts_then_its_usage() {
    let events = api()
        .decode(
            r#"data: {"candidates":[{"content":{"parts":[{"text":"Hel"},{"text":"lo"}],"role":"model"},"index":0}],"usageMetadata":{"promptTokenCount":8,"candidatesTokenCount":2,"totalTokenCount":10},"modelVersion":"gemini-2.5-flash"}"#,
        )
        .expect("decodes");
    assert_eq!(
        events,
        vec![
            StreamEvent::Text("Hel".to_owned()),
            StreamEvent::Text("lo".to_owned()),
            StreamEvent::Usage {
                input: 8,
                output: 2
            },
        ]
    );
}

#[test]
fn a_chunk_without_candidates_still_yields_its_usage() {
    let events = api()
        .decode(r#"data: {"usageMetadata":{"promptTokenCount":8,"candidatesTokenCount":5}}"#)
        .expect("decodes");
    assert_eq!(
        events,
        vec![StreamEvent::Usage {
            input: 8,
            output: 5
        }]
    );
}

#[test]
fn blank_lines_and_other_sse_fields_carry_nothing() {
    let api = api();
    for line in ["", "event: ping", ": comment"] {
        assert!(api.decode(line).expect("decodes").is_empty(), "{line:?}");
    }
}

#[tokio::test]
async fn the_request_puts_the_key_in_the_header_and_the_history_in_contents() {
    let history = vec![
        ChatMessage::new(Role::User, "hi"),
        ChatMessage::new(Role::Assistant, "hello"),
        ChatMessage::new(Role::User, "more"),
    ];
    let request = api()
        .request(
            &reqwest::Client::new(),
            "gemini-2.5-pro",
            &history,
            Some("Use the project conventions."),
        )
        .build()
        .expect("builds");
    assert_eq!(
        request.url().as_str(),
        "https://example.test/v1beta/models/gemini-2.5-pro:streamGenerateContent?alt=sse"
    );
    assert!(
        !request.url().as_str().contains("AIza"),
        "the key stays out of the URL"
    );
    assert_eq!(
        request.headers()["x-goog-api-key"].to_str().expect("ascii"),
        "AIza-test"
    );
    let body: serde_json::Value =
        serde_json::from_slice(request.body().and_then(|b| b.as_bytes()).expect("a body"))
            .expect("json");
    assert_eq!(
        body["systemInstruction"]["parts"][0]["text"],
        "Use the project conventions."
    );
    assert_eq!(body["contents"][1]["role"], "model");
    assert_eq!(body["contents"][2]["parts"][0]["text"], "more");
}
