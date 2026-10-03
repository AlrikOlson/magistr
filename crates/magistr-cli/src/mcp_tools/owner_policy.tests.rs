use super::*;
use crate::mcp_bridge::{ClientSession, Inbound};

#[test]
fn owner_loss_keeps_owner_discovery_and_reports_unknown_effect_once() {
    let mut session = ClientSession::new("workflow".to_owned(), Policy);
    let init = json!({"jsonrpc":"2.0", "id":1, "method":"initialize", "params":{"protocolVersion":"2025-06-18"}});
    session.inbound(&init.to_string(), true);
    let info = json!({"protocolVersion":"2025-06-18", "serverInfo":{"name":"served-owner", "version":"fixture"}, "capabilities":{"tools":{"listChanged":true}}});
    session.outbound(&json!({"jsonrpc":"2.0", "id":1, "result":info}).to_string());
    session.inbound(
        &json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}).to_string(),
        true,
    );
    let tools = json!({"tools":[{"name":"owned-tool"}]});
    session.outbound(&json!({"jsonrpc":"2.0","id":2,"result":tools}).to_string());
    let change = json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"ship_record","arguments":{"description":"change"}}});
    let Inbound::Forward(frame) = session.inbound(&change.to_string(), true) else {
        panic!("forwarded");
    };
    assert_eq!(frame, change.to_string(), "workflow frames are opaque");
    let lost = session.owner_lost("fixture owner exited");
    assert_eq!(lost.len(), 1);
    let refusal = &lost[0]["result"];
    assert_eq!(refusal["isError"], true);
    assert_eq!(refusal["structuredContent"]["effect"], "unknown");
    assert_eq!(
        refusal["structuredContent"]["recovery"]["action"],
        "read_back"
    );
    let text: Value =
        serde_json::from_str(refusal["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(text, refusal["structuredContent"]);
    assert!(session.owner_lost("again").is_empty());
    assert_eq!(session.answer_unavailable(&init, "gone")["result"], info);
    assert_eq!(
        session.answer_unavailable(&json!({"id":4,"method":"tools/list"}), "gone")["result"],
        tools
    );
    let (_, replay, _) = session.replay().expect("initialization replayed");
    assert_eq!(replay["method"], "initialize");
    assert_eq!(replay["params"], init["params"]);
    assert_eq!(
        session.tools_changed().unwrap()["method"],
        "notifications/tools/list_changed"
    );
}
