use super::*;
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
