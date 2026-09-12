use super::*;

fn session() -> ClientSession {
    ClientSession::new("bridge-test".to_owned())
}

fn call(id: u64, meta: Option<Value>) -> String {
    let mut params = json!({"name": "read", "arguments": {"file": "a.txt"}});
    if let Some(meta) = meta {
        params["_meta"] = meta;
    }
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": params}).to_string()
}

#[test]
fn a_call_without_identity_is_stamped_and_an_explicit_one_is_kept() {
    let mut session = session();
    let Inbound::Forward(stamped) = session.inbound(&call(1, None), true) else {
        panic!("a linked call is forwarded");
    };
    let stamped: Value = serde_json::from_str(&stamped).unwrap();
    assert_eq!(
        stamped["params"]["_meta"]["workbench/sessionId"],
        "bridge-test"
    );

    let explicit = call(2, Some(json!({"workbench/sessionId": "alice"})));
    let Inbound::Forward(kept) = session.inbound(&explicit, true) else {
        panic!("a linked call is forwarded");
    };
    assert_eq!(kept, explicit, "an explicit identity is forwarded verbatim");
}

#[test]
fn unmodified_frames_are_forwarded_byte_for_byte() {
    let mut session = session();
    let raw = r#"{"jsonrpc":"2.0","id":7,"method":"tools/list","params":{ }}"#;
    let Inbound::Forward(forwarded) = session.inbound(raw, true) else {
        panic!("forwarded");
    };
    assert_eq!(forwarded, raw);
}

#[test]
fn owner_loss_answers_every_outstanding_request_once() {
    let mut session = session();
    session.inbound(&call(1, None), true);
    session.inbound(
        &json!({"jsonrpc": "2.0", "id": "x", "method": "tools/list"}).to_string(),
        true,
    );
    session.inbound(
        &json!({"jsonrpc": "2.0", "method": "notifications/cancelled"}).to_string(),
        true,
    );
    let answers = session.owner_lost("owner crashed");
    assert_eq!(answers.len(), 2);
    let refused = answers
        .iter()
        .find(|answer| answer["id"] == 1)
        .expect("the tool call is answered");
    let envelope = &refused["result"]["structuredContent"];
    assert_eq!(envelope["ok"], false);
    assert_eq!(envelope["error"]["code"], "ENGINE_UNAVAILABLE");
    assert_eq!(envelope["error"]["recovery"], "retry");
    let text: Value =
        serde_json::from_str(refused["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        &text, envelope,
        "text and structured content are one envelope"
    );
    let listing = answers
        .iter()
        .find(|answer| answer["id"] == "x")
        .expect("the listing is answered");
    assert_eq!(
        listing["error"]["code"], -32000,
        "no listing was cached yet"
    );
    assert!(session.owner_lost("again").is_empty(), "answered once");
}

#[test]
fn the_last_tool_listing_answers_while_no_owner_is_reachable() {
    let mut session = session();
    session.inbound(
        &json!({"jsonrpc": "2.0", "id": 3, "method": "tools/list"}).to_string(),
        true,
    );
    session.outbound(
        &json!({"jsonrpc": "2.0", "id": 3, "result": {"tools": [{"name": "read"}]}}).to_string(),
    );
    let Inbound::Unanswered(request) = session.inbound(
        &json!({"jsonrpc": "2.0", "id": 4, "method": "tools/list"}).to_string(),
        false,
    ) else {
        panic!("an unlinked request is answered locally");
    };
    let answer = session.answer_unavailable(&request, "gone");
    assert_eq!(answer["id"], 4);
    assert_eq!(answer["result"]["tools"][0]["name"], "read");
    let ping = session.answer_unavailable(
        &json!({"jsonrpc": "2.0", "id": 5, "method": "ping"}),
        "gone",
    );
    assert_eq!(ping["result"], json!({}));
}

#[test]
fn initialize_is_recorded_and_replayed_under_a_bridge_id() {
    let mut session = session();
    assert!(!session.initialized());
    assert!(session.replay().is_none());
    let params = json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}});
    session.inbound(
        &json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": params}).to_string(),
        true,
    );
    assert!(session.initialized());
    let (id, initialize, initialized) = session.replay().expect("replay");
    assert_eq!(id, "magistr-bridge-replay-1");
    assert_eq!(initialize["params"], params);
    assert_eq!(initialized["method"], "notifications/initialized");
    let (second, ..) = session.replay().expect("replay again");
    assert_eq!(second, "magistr-bridge-replay-2");
}

#[test]
fn ids_are_compared_as_json_so_numbers_and_strings_differ() {
    let mut session = session();
    session.inbound(&call(1, None), true);
    session.outbound(&json!({"jsonrpc": "2.0", "id": "1", "result": {}}).to_string());
    assert_eq!(
        session.owner_lost("gone").len(),
        1,
        "\"1\" did not settle 1"
    );
}

#[test]
fn unlinked_notifications_and_non_json_lines_are_dropped() {
    let mut session = session();
    assert!(matches!(
        session.inbound(
            &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string(),
            false
        ),
        Inbound::Dropped
    ));
    assert!(matches!(
        session.inbound("not json", false),
        Inbound::Dropped
    ));
    assert!(matches!(
        session.inbound("not json", true),
        Inbound::Forward(line) if line == "not json"
    ));
}
