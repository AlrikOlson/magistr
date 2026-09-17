use super::*;

fn session() -> ClientSession {
    ClientSession::new("bridge-test".to_owned(), Capability::new(true))
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
        listing["result"],
        Capability::new(true).core_listing(),
        "no owner ever listed, so the shared definition answers"
    );
    assert!(session.owner_lost("again").is_empty(), "answered once");
}

#[test]
fn initialize_and_the_listing_are_answered_locally_before_any_owner() {
    let mut session = session();
    let params = json!({"protocolVersion": "2026-07-28", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}});
    let Inbound::Unanswered(request) = session.inbound(
        &json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": params}).to_string(),
        false,
    ) else {
        panic!("an unlinked initialize is answered locally");
    };
    let answer = session.answer_unavailable(&request, "no owner");
    assert_eq!(answer["id"], 0);
    assert_eq!(
        answer["result"],
        serde_json::to_value(Capability::initialize(&ProtocolVersion::V_2026_07_28)).unwrap()
    );
    assert_eq!(answer["result"]["protocolVersion"], "2025-06-18");
    assert!(
        session.initialized(),
        "the later owner is initialized on the client's behalf"
    );
    let Inbound::Unanswered(listing) = session.inbound(
        &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}).to_string(),
        false,
    ) else {
        panic!("an unlinked listing is answered locally");
    };
    let answer = session.answer_unavailable(&listing, "no owner");
    assert_eq!(answer["result"], Capability::new(true).core_listing());
    let call = session.answer_unavailable(
        &serde_json::from_str(&call(2, None)).unwrap(),
        "run `magistr workspace migrate`",
    );
    let envelope = &call["result"]["structuredContent"];
    assert_eq!(envelope["error"]["code"], "ENGINE_UNAVAILABLE");
    assert!(
        envelope["error"]["message"]
            .as_str()
            .unwrap()
            .contains("magistr workspace migrate"),
        "{envelope}"
    );
}

#[test]
fn an_initialize_without_a_protocol_version_is_invalid_params() {
    let mut session = session();
    let Inbound::Unanswered(request) = session.inbound(
        &json!({"jsonrpc": "2.0", "id": 9, "method": "initialize", "params": {"capabilities": {}}})
            .to_string(),
        false,
    ) else {
        panic!("answered locally");
    };
    let answer = session.answer_unavailable(&request, "no owner");
    assert_eq!(answer["error"]["code"], -32602, "{answer}");
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
