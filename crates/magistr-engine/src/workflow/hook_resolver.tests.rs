use super::*;
use crate::workflow::{HookAction, LifecycleEvent, LifecycleHook, WorkflowPhase};
use serde_json::json;

fn hook(event: LifecycleEvent, tool: &str) -> LifecycleHook {
    LifecycleHook {
        event,
        action: HookAction::McpTool {
            tool: tool.to_owned(),
            arguments: json!({}),
        },
        description: None,
        on_failure: crate::workflow::OnFailure::default(),
    }
}

#[test]
fn empty_config_returns_no_hooks() {
    let hooks: Vec<LifecycleHook> = vec![];
    let resolved = HookResolver::resolve(&LifecycleEvent::OnIterationStart, &hooks);
    assert!(resolved.is_empty());
}

#[test]
fn returns_matching_hooks_only() {
    let hooks = vec![
        hook(
            LifecycleEvent::OnEnter(WorkflowPhase::Researching),
            "tool_a",
        ),
        hook(LifecycleEvent::OnIterationStart, "tool_b"),
        hook(LifecycleEvent::OnEnter(WorkflowPhase::Executing), "tool_c"),
    ];

    let resolved =
        HookResolver::resolve(&LifecycleEvent::OnEnter(WorkflowPhase::Researching), &hooks);
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].display_name(), "tool_a");
}

#[test]
fn preserves_declaration_order() {
    let hooks = vec![
        hook(LifecycleEvent::OnGatePass, "first"),
        hook(LifecycleEvent::OnGatePass, "second"),
        hook(LifecycleEvent::OnGatePass, "third"),
    ];

    let resolved = HookResolver::resolve(&LifecycleEvent::OnGatePass, &hooks);
    assert_eq!(resolved.len(), 3);
    assert_eq!(resolved[0].display_name(), "first");
    assert_eq!(resolved[1].display_name(), "second");
    assert_eq!(resolved[2].display_name(), "third");
}

#[test]
fn no_match_returns_empty() {
    let hooks = vec![
        hook(LifecycleEvent::OnGatePass, "tool_a"),
        hook(LifecycleEvent::OnEnter(WorkflowPhase::Planning), "tool_b"),
    ];

    let resolved = HookResolver::resolve(&LifecycleEvent::OnGateFail, &hooks);
    assert!(resolved.is_empty());
}

#[test]
fn resolved_hook_carries_arguments_and_description() {
    let hooks = vec![LifecycleHook {
        event: LifecycleEvent::OnIterationEnd,
        action: HookAction::McpTool {
            tool: "my_tool".to_owned(),
            arguments: json!({"key": "value"}),
        },
        description: Some("run after iteration".to_owned()),
        on_failure: crate::workflow::OnFailure::default(),
    }];

    let resolved = HookResolver::resolve(&LifecycleEvent::OnIterationEnd, &hooks);
    assert_eq!(resolved.len(), 1);
    match &resolved[0].action {
        HookAction::McpTool { arguments, .. } => {
            assert_eq!(arguments, &json!({"key": "value"}));
        }
        _ => panic!("expected McpTool"),
    }
    assert_eq!(
        resolved[0].description.as_deref(),
        Some("run after iteration")
    );
}

#[test]
fn on_enter_different_phases_do_not_match() {
    let hooks = vec![hook(
        LifecycleEvent::OnEnter(WorkflowPhase::Verifying),
        "verifier",
    )];

    let resolved =
        HookResolver::resolve(&LifecycleEvent::OnEnter(WorkflowPhase::Executing), &hooks);
    assert!(resolved.is_empty());
}

#[test]
fn resolves_shell_command_hooks() {
    let hooks = vec![LifecycleHook {
        event: LifecycleEvent::OnEnter(WorkflowPhase::Verifying),
        action: HookAction::ShellCommand {
            command: "cargo fmt".to_owned(),
            timeout_secs: Some(60),
        },
        description: Some("format code".to_owned()),
        on_failure: crate::workflow::OnFailure::FailFast,
    }];

    let resolved =
        HookResolver::resolve(&LifecycleEvent::OnEnter(WorkflowPhase::Verifying), &hooks);
    assert_eq!(resolved.len(), 1);
    assert!(resolved[0].action.is_shell_command());
    assert_eq!(resolved[0].display_name(), "cargo fmt");
}
