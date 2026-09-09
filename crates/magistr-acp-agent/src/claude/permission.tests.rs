use agent_client_protocol::schema::v1::{PermissionOptionId, SelectedPermissionOutcome};

use super::*;

/// The `can_use_tool` request recorded in tool_permission_allow.ndjson.
fn recorded() -> Value {
    serde_json::json!({
        "subtype": "can_use_tool",
        "tool_name": "Write",
        "display_name": "Write",
        "input": {"file_path": "/work/allow.txt", "content": "hello"},
        "description": "allow.txt",
        "permission_suggestions": [{"type": "setMode", "mode": "acceptEdits", "destination": "session"}],
        "tool_use_id": "toolu_fixture00000030"
    })
}

fn selected(id: &str) -> RequestPermissionOutcome {
    RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(PermissionOptionId::new(id)))
}

#[test]
fn the_ask_names_the_tool_call_it_follows() {
    let ask = PermissionAsk::parse(&recorded()).expect("parses");
    let call = ask.tool_call();
    assert_eq!(call.tool_call_id.to_string(), "toolu_fixture00000030");
    assert_eq!(
        call.fields.title.as_deref(),
        Some("Write"),
        "the raw tool name, never the display name: policies key on it"
    );
    assert_eq!(
        call.fields.kind,
        Some(agent_client_protocol::schema::v1::ToolKind::Edit)
    );
    assert_eq!(
        call.fields.raw_input,
        Some(serde_json::json!({"file_path": "/work/allow.txt", "content": "hello"}))
    );
    let kinds: Vec<PermissionOptionKind> = ask.options().into_iter().map(|o| o.kind).collect();
    assert_eq!(
        kinds,
        [
            PermissionOptionKind::AllowOnce,
            PermissionOptionKind::AllowAlways,
            PermissionOptionKind::RejectOnce
        ]
    );
}

#[test]
fn an_mcp_tool_ask_keeps_its_full_mcp_name() {
    let ask = PermissionAsk::parse(&serde_json::json!({
        "subtype": "can_use_tool",
        "tool_name": "mcp__magistr__roadmap_status",
        "display_name": "Roadmap Status",
        "input": {},
        "tool_use_id": "toolu_1"
    }))
    .expect("parses");
    let call = ask.tool_call();
    assert_eq!(
        call.fields.title.as_deref(),
        Some("mcp__magistr__roadmap_status")
    );
    assert_eq!(
        call.fields.kind,
        Some(agent_client_protocol::schema::v1::ToolKind::Other)
    );
}

#[test]
fn an_ask_without_a_tool_use_id_is_refused() {
    assert_eq!(
        PermissionAsk::parse(&serde_json::json!({"subtype": "can_use_tool", "tool_name": "Bash"})),
        None
    );
    assert_eq!(PermissionAsk::parse(&serde_json::json!("nope")), None);
}

#[test]
fn allow_echoes_the_input_and_always_adds_the_suggestions() {
    let ask = PermissionAsk::parse(&recorded()).expect("parses");
    assert_eq!(
        ask.answer(&selected(PermissionAsk::ALLOW)),
        serde_json::json!({"behavior": "allow", "updatedInput": {"file_path": "/work/allow.txt", "content": "hello"}})
    );
    assert_eq!(
        ask.answer(&selected(PermissionAsk::ALLOW_ALWAYS)),
        serde_json::json!({
            "behavior": "allow",
            "updatedInput": {"file_path": "/work/allow.txt", "content": "hello"},
            "updatedPermissions": [{"type": "setMode", "mode": "acceptEdits", "destination": "session"}]
        })
    );
}

#[test]
fn reject_and_cancel_deny_and_cancel_also_interrupts() {
    let ask = PermissionAsk::parse(&recorded()).expect("parses");
    let rejected = ask.answer(&selected(PermissionAsk::REJECT));
    assert_eq!(rejected["behavior"], "deny");
    assert!(
        rejected["message"]
            .as_str()
            .is_some_and(|m| m.contains("rejected"))
    );
    assert!(rejected.get("interrupt").is_none());

    let stranger = ask.answer(&selected("something-else"));
    assert_eq!(stranger["behavior"], "deny");

    let cancelled = ask.answer(&RequestPermissionOutcome::Cancelled);
    assert_eq!(cancelled["behavior"], "deny");
    assert_eq!(cancelled["interrupt"], true);
}
