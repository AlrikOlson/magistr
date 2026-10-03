use super::*;
use rmcp::model::ServerCapabilities;
use serde_json::json;

fn initialize_message(version: &str) -> ClientJsonRpcMessage {
    serde_json::from_value(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": version,
            "capabilities": {},
            "clientInfo": {"name": "probe", "version": "0"}
        }
    }))
    .expect("initialize message")
}

fn requested_version(message: &ClientJsonRpcMessage) -> String {
    let ClientJsonRpcMessage::Request(request) = message else {
        panic!("not a request");
    };
    let ClientRequest::InitializeRequest(init) = &request.request else {
        panic!("not initialize");
    };
    init.params.protocol_version.as_str().to_owned()
}

#[test]
fn the_modern_era_is_clamped_to_the_served_version() {
    let mut message = initialize_message("2026-07-28");
    clamp(&mut message);
    assert_eq!(requested_version(&message), "2025-06-18");
}

#[test]
fn an_unknown_future_version_is_clamped_too() {
    let mut message = initialize_message("2027-01-01");
    clamp(&mut message);
    assert_eq!(requested_version(&message), "2025-06-18");
}

#[test]
fn older_versions_pass_through_untouched() {
    for version in ["2025-06-18", "2025-03-26", "2024-11-05", "2025-11-25"] {
        let mut message = initialize_message(version);
        clamp(&mut message);
        assert_eq!(requested_version(&message), version, "{version}");
    }
}

#[test]
fn a_modern_per_request_meta_version_is_clamped_on_any_request() {
    let mut message: ClientJsonRpcMessage = serde_json::from_value(json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "tools/list",
        "params": {"_meta": {
            "io.modelcontextprotocol/protocolVersion": "2026-07-28",
            "io.modelcontextprotocol/clientInfo": {"name": "probe", "version": "0"}
        }}
    }))
    .expect("tools/list message");
    clamp(&mut message);
    let ClientJsonRpcMessage::Request(request) = &message else {
        panic!("not a request");
    };
    assert_eq!(
        request.request.get_meta().protocol_version(),
        Some(ProtocolVersion::V_2025_06_18)
    );
}

#[test]
fn non_request_messages_are_left_alone() {
    let mut message: ClientJsonRpcMessage = serde_json::from_value(json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }))
    .expect("notification");
    let before = format!("{message:?}");
    clamp(&mut message);
    assert_eq!(format!("{message:?}"), before);
}

fn discover_message(id: u32) -> ClientJsonRpcMessage {
    serde_json::from_value(json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "server/discover",
        "params": {"_meta": {
            "io.modelcontextprotocol/protocolVersion": "2026-07-28",
            "io.modelcontextprotocol/clientInfo": {"name": "probe", "version": "0"},
            "io.modelcontextprotocol/clientCapabilities": {}
        }}
    }))
    .expect("discover message")
}

fn opening() -> Opening {
    Opening::new(DiscoverResult::new(
        vec![ProtocolVersion::V_2025_06_18],
        ServerCapabilities::builder().enable_tools().build(),
    ))
}

#[test]
fn a_discover_before_initialize_is_answered_by_the_cap() {
    let mut opening = opening();
    let reply = opening
        .reply_to(&discover_message(0))
        .expect("the cap answers the opening discover");
    let reply = serde_json::to_value(reply).expect("reply json");
    assert_eq!(reply["id"], 0);
    assert_eq!(reply["result"]["supportedVersions"], json!(["2025-06-18"]));
}

#[test]
fn every_discover_before_initialize_is_answered() {
    let mut opening = opening();
    assert!(opening.reply_to(&discover_message(0)).is_some());
    assert!(opening.reply_to(&discover_message(1)).is_some());
}

#[test]
fn a_discover_after_initialize_is_left_to_the_server() {
    let mut opening = opening();
    assert!(
        opening
            .reply_to(&initialize_message("2025-06-18"))
            .is_none()
    );
    assert!(opening.reply_to(&discover_message(2)).is_none());
}

#[test]
fn a_session_opened_by_another_request_is_left_to_the_server() {
    let mut opening = opening();
    let tools: ClientJsonRpcMessage = serde_json::from_value(json!({
        "jsonrpc": "2.0", "id": 0, "method": "tools/list"
    }))
    .expect("tools/list message");
    assert!(opening.reply_to(&tools).is_none());
    assert!(opening.reply_to(&discover_message(1)).is_none());
}

#[test]
fn a_discover_without_its_required_meta_is_left_to_the_server() {
    let mut opening = opening();
    let bare: ClientJsonRpcMessage = serde_json::from_value(json!({
        "jsonrpc": "2.0", "id": 0, "method": "server/discover"
    }))
    .expect("bare discover message");
    assert!(opening.reply_to(&bare).is_none());
}

#[test]
fn a_notification_does_not_open_the_session() {
    let mut opening = opening();
    let note: ClientJsonRpcMessage = serde_json::from_value(json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }))
    .expect("notification");
    assert!(opening.reply_to(&note).is_none());
    assert!(opening.reply_to(&discover_message(0)).is_some());
}
