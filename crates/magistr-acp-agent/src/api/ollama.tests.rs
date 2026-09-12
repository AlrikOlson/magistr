use super::*;

#[test]
fn done_is_terminal_even_without_usage_and_length_is_distinct() {
    for (reason, stop) in [
        ("stop", StopReason::EndTurn),
        ("length", StopReason::MaxTokens),
    ] {
        let line = format!(r#"{{"done":true,"done_reason":"{reason}"}}"#);
        assert_eq!(
            api().decode(&line).expect("terminal"),
            vec![StreamEvent::Finish(stop)]
        );
    }
    assert!(
        api()
            .decode(r#"{"done":false,"done_reason":"stop"}"#)
            .expect("not finished")
            .is_empty()
    );
    assert_eq!(
        api().decode(r#"{"done":true}"#).expect("legacy done"),
        vec![StreamEvent::Finish(StopReason::EndTurn)]
    );
}

#[test]
fn errors_and_unknown_done_reasons_fail() {
    for reason in ["load", "unload", "future", ""] {
        let line = format!(r#"{{"done":true,"done_reason":"{reason}"}}"#);
        assert!(api().decode(&line).is_err(), "{line}");
    }
    assert!(
        api()
            .decode(r#"{"error":"overloaded"}"#)
            .expect_err("error")
            .to_string()
            .contains("overloaded")
    );
}

fn api() -> Ollama {
    Ollama {
        base_url: "http://127.0.0.1:11434".to_owned(),
        models: vec![ModelSpec::new(
            "llama3.1:latest",
            "llama3.1:latest",
            Ollama::WINDOW,
            Pricing::FREE,
        )],
    }
}

#[test]
fn a_host_without_a_scheme_gets_http_and_a_trailing_slash_goes() {
    assert_eq!(normalize_host("localhost:11434"), "http://localhost:11434");
    assert_eq!(normalize_host("http://box:11434/"), "http://box:11434");
    assert_eq!(
        normalize_host(" https://ollama.example "),
        "https://ollama.example"
    );
}

#[test]
fn a_content_line_is_text_and_done_carries_usage_and_finish() {
    let api = api();
    assert_eq!(
        api.decode(
            r#"{"model":"llama3.1","message":{"role":"assistant","content":"Hel"},"done":false}"#
        )
        .expect("decodes"),
        vec![StreamEvent::Text("Hel".to_owned())]
    );
    assert_eq!(
        api.decode(
            r#"{"model":"llama3.1","message":{"role":"assistant","content":""},"done":true,"done_reason":"stop","prompt_eval_count":9,"eval_count":4}"#
        )
        .expect("decodes"),
        vec![StreamEvent::Usage {
            input: 9,
            output: 4
        }, StreamEvent::Finish(StopReason::EndTurn)]
    );
}

#[test]
fn a_done_line_keeps_text_without_inventing_missing_token_counts() {
    let events = api()
        .decode(r#"{"message":{"role":"assistant","content":"."},"done":true,"eval_count":1}"#)
        .expect("decodes");
    assert_eq!(
        events,
        vec![
            StreamEvent::Text(".".to_owned()),
            StreamEvent::Finish(StopReason::EndTurn)
        ]
    );
}

#[test]
fn a_blank_line_carries_nothing_and_a_malformed_one_is_refused() {
    let api = api();
    assert!(api.decode("  ").expect("decodes").is_empty());
    let err = api.decode("{oops").expect_err("malformed");
    assert!(err.to_string().contains("{oops"), "{err}");
}

#[tokio::test]
async fn the_request_streams_the_history_to_api_chat() {
    let history = vec![
        ChatMessage::new(Role::User, "hi"),
        ChatMessage::new(Role::Assistant, "hello"),
    ];
    let request = api()
        .request(&reqwest::Client::new(), "llama3.1:latest", &history)
        .build()
        .expect("builds");
    assert_eq!(request.url().as_str(), "http://127.0.0.1:11434/api/chat");
    let body: serde_json::Value =
        serde_json::from_slice(request.body().and_then(|b| b.as_bytes()).expect("a body"))
            .expect("json");
    assert_eq!(body["model"], "llama3.1:latest");
    assert_eq!(body["stream"], true);
    assert_eq!(body["messages"][1]["role"], "assistant");
}

#[tokio::test]
async fn a_daemon_that_is_not_there_is_refused_with_the_host_and_the_remedy() {
    let err = Ollama::discover_at("127.0.0.1:9")
        .await
        .expect_err("nothing listens on port 9");
    let message = err.to_string();
    assert!(message.contains("http://127.0.0.1:9"), "{message}");
    assert!(message.contains("ollama serve"), "{message}");
}
