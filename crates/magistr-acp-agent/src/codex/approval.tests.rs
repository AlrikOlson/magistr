use super::*;
use agent_client_protocol::schema::v1::SelectedPermissionOutcome;

fn selected(option: &str) -> RequestPermissionOutcome {
    RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(option.to_owned()))
}

#[test]
fn mcp_tool_approval_preserves_scope_and_never_grants_persistent_access() {
    let params = json!({
        "threadId": "thread-1", "turnId": "turn-1", "serverName": "workspace",
        "mode": "form", "message": "Allow this file read?",
        "requestedSchema": {"type": "object", "properties": {}},
        "_meta": {"codex_approval_kind": "mcp_tool_call", "tool_params": {"file": "sample.txt"},
                  "persist": ["session", "always"]}
    });
    assert!(Approval::is_tool_elicitation(&params));
    let approval = Approval::elicitation(&json!(42), &params).unwrap();
    assert_eq!(approval.tool_call().fields.raw_input, Some(params.clone()));
    assert_eq!(
        approval.answer(&selected("allow")),
        json!({"action":"accept","content":{}})
    );
    assert_eq!(
        approval.answer(&selected("reject")),
        json!({"action":"decline"})
    );
    assert_eq!(
        approval.answer(&selected("allow_always")),
        json!({"action":"decline"})
    );
    assert_eq!(
        approval.answer(&RequestPermissionOutcome::Cancelled),
        json!({"action":"cancel"})
    );
    assert!(
        !approval
            .options()
            .iter()
            .any(|option| option.kind == PermissionOptionKind::AllowAlways)
    );
    for change in [
        json!({"mode": "url"}),
        json!({"_meta": {}}),
        json!({"turnId": null}),
        json!({"requestedSchema":{"type":"object","properties":{"secret":{"type":"string"}}}}),
        json!({"requestedSchema":{"type":"object","properties":{},"required":["secret"]}}),
    ] {
        let mut unsupported = params.clone();
        unsupported
            .as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        assert!(
            !Approval::is_tool_elicitation(&unsupported),
            "{unsupported}"
        );
        assert!(Approval::elicitation(&json!(42), &unsupported).is_err());
    }
}

#[test]
fn command_decisions_preserve_the_clients_selection_and_cancellation() {
    let approval = Approval::parse(
        "item/commandExecution/requestApproval",
        &json!({
            "itemId": "command-1",
            "command": "printf hello",
            "availableDecisions": ["accept", "acceptForSession", "decline", "cancel"]
        }),
        None,
    )
    .expect("command approval");
    for (option, decision) in [
        ("allow", "accept"),
        ("allow_always", "acceptForSession"),
        ("reject", "decline"),
    ] {
        assert_eq!(
            approval.answer(&selected(option)),
            json!({"decision": decision})
        );
    }
    assert_eq!(
        approval.answer(&RequestPermissionOutcome::Cancelled),
        json!({"decision": "cancel"})
    );
}

#[test]
fn restricted_decisions_never_send_an_unadvertised_grant() {
    let approval = Approval::parse(
        "item/commandExecution/requestApproval",
        &json!({
            "itemId": "command-1",
            "availableDecisions": ["accept", {"acceptWithExecpolicyAmendment": {
                "execpolicy_amendment": ["/bin/sh", "-lc", "printf hello"]
            }}, "cancel"]
        }),
        None,
    )
    .expect("restricted approval");
    assert!(
        approval
            .options()
            .iter()
            .all(|option| option.kind != PermissionOptionKind::AllowAlways),
        "a policy amendment is not permission to allow the whole session"
    );
    assert_eq!(
        approval.answer(&selected("allow")),
        json!({"decision": "accept"})
    );
    for option in ["reject", "allow_always", "not-an-option"] {
        assert_eq!(
            approval.answer(&selected(option)),
            json!({"decision": "decline"}),
            "{option} must never grant an unadvertised approval"
        );
    }
}

#[test]
fn cancel_only_requests_do_not_offer_or_execute_an_allow_option() {
    let approval = Approval::parse(
        "item/commandExecution/requestApproval",
        &json!({"itemId": "command-1", "availableDecisions": ["cancel"]}),
        None,
    )
    .expect("cancel-only approval");
    assert!(approval.options().iter().all(|option| {
        option.kind != PermissionOptionKind::AllowOnce
            && option.kind != PermissionOptionKind::AllowAlways
    }));
    assert_eq!(
        approval.answer(&selected("allow")),
        json!({"decision": "decline"})
    );
}

#[test]
fn file_approval_keeps_the_patch_and_scope_for_the_clients_policy() {
    let params = json!({"itemId": "edit-1", "threadId": "thread-1", "turnId": "turn-1", "reason": "update greeting", "grantRoot": "/workspace"});
    let item = json!({"id": "edit-1", "type": "fileChange", "changes": [{
        "path": "/workspace/greeting.txt", "kind": {"type": "update"},
        "diff": "-hello\n+goodbye"
    }]});
    let approval = Approval::parse("item/fileChange/requestApproval", &params, Some(&item))
        .expect("file approval");
    let call = approval.tool_call();
    assert_eq!(call.tool_call_id.to_string(), "edit-1");
    assert_eq!(call.fields.kind, Some(ToolKind::Edit));
    let raw = call.fields.raw_input.expect("policy input");
    assert_eq!(raw["grantRoot"], "/workspace");
    assert_eq!(raw["item"], item);
    assert_eq!(
        approval.answer(&selected("allow_always")),
        json!({"decision": "acceptForSession"})
    );
}

#[test]
fn approval_requires_an_item_identity_before_the_client_is_asked() {
    for params in [json!({}), json!({"itemId": ""}), json!({"itemId": 17})] {
        assert!(Approval::parse("item/commandExecution/requestApproval", &params, None).is_err());
    }
}

#[test]
fn file_approval_exposes_every_patch_path_and_move_destination_to_policy() {
    let params = json!({"itemId":"patch-1"});
    let item = json!({"id":"patch-1","type":"fileChange","changes":[
        {"path":"src/allowed.rs","kind":{"type":"update"},"diff":"patch"},
        {"path":".magistr/roadmap.yaml","kind":{"type":"delete"},"diff":"patch"},
        {"path":"src/old.rs","kind":{"type":"update","move_path":".magistr/project.json"},"diff":"patch"}
    ]});
    let approval = Approval::parse("item/fileChange/requestApproval", &params, Some(&item))
        .expect("file approval");
    let input = approval.tool_call().fields.raw_input.expect("policy input");
    assert_eq!(
        input["paths"],
        json!([
            "src/allowed.rs",
            ".magistr/roadmap.yaml",
            "src/old.rs",
            ".magistr/project.json"
        ])
    );
    assert_eq!(input["item"], item);
}

#[test]
fn file_approval_without_complete_patch_paths_is_refused_before_policy() {
    let params = json!({"itemId":"patch-1"});
    assert!(Approval::parse("item/fileChange/requestApproval", &params, None).is_err());
    for changes in [
        json!([]),
        json!([{"path":"allowed.rs"},{}]),
        json!([{"path":""}]),
        json!([{"path":"allowed.rs","kind":{"type":"update","move_path":42}}]),
    ] {
        let item = json!({"id":"patch-1","type":"fileChange","changes":changes});
        assert!(
            Approval::parse("item/fileChange/requestApproval", &params, Some(&item)).is_err(),
            "incomplete patch scope must not receive an automatic edit grant: {item}"
        );
    }
}
