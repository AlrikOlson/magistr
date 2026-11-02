use magistr_workflow::run::engine::RunEngine;
use magistr_workflow::run::mcp::RunService;

fn service() -> RunService {
    let engine = RunEngine::new("test-abc123".into());
    RunService::new(engine)
}

#[test]
fn tools_list_has_14_canonical_entries() {
    let svc = service();
    let tools = svc.list_tools_view();
    assert_eq!(
        tools.len(),
        14,
        "expected 14 run tools, including explicit check compaction, got {}",
        tools.len()
    );
    let run_count = tools.iter().filter(|t| t.name.starts_with("run_")).count();
    assert_eq!(run_count, 7);
}

#[test]
fn every_tool_has_annotations() {
    let svc = service();
    for tool in svc.list_tools_view() {
        assert!(
            tool.annotations.is_some(),
            "tool '{}' missing annotations",
            tool.name
        );
    }
}

#[test]
fn every_tool_has_output_schema() {
    let svc = service();
    for tool in svc.list_tools_view() {
        assert!(
            tool.output_schema.is_some(),
            "tool '{}' missing output_schema",
            tool.name
        );
    }
}

#[test]
fn destructive_tools_are_marked() {
    let svc = service();
    let destructive_names = ["run_reset", "run_compact_checks"];
    for tool in svc.list_tools_view() {
        let annotations = tool.annotations.as_ref().unwrap();
        if destructive_names.contains(&tool.name.as_ref()) {
            assert_eq!(
                annotations.destructive_hint,
                Some(true),
                "tool '{}' should be marked destructive",
                tool.name
            );
        } else {
            assert_ne!(
                annotations.destructive_hint,
                Some(true),
                "tool '{}' should NOT be marked destructive",
                tool.name
            );
        }
    }
}

#[test]
fn read_only_tools_are_marked() {
    let svc = service();
    let read_only = [
        "run_status",
        "run_export",
        // A gate poll reads the workspace and writes nothing.
        "approval_wait",
    ];
    for tool in svc.list_tools_view() {
        let annotations = tool.annotations.as_ref().unwrap();
        if read_only.contains(&tool.name.as_ref()) {
            assert_eq!(
                annotations.read_only_hint,
                Some(true),
                "tool '{}' should be read-only",
                tool.name
            );
        } else {
            assert_ne!(
                annotations.read_only_hint,
                Some(true),
                "tool '{}' should NOT be read-only",
                tool.name
            );
        }
    }
}

#[test]
fn every_tool_has_title() {
    let svc = service();
    for tool in svc.list_tools_view() {
        let annotations = tool.annotations.as_ref().unwrap();
        assert!(
            annotations.title.is_some(),
            "tool '{}' missing title annotation",
            tool.name
        );
    }
}

#[test]
fn every_tool_name_is_claimed_by_the_run_family() {
    let svc = service();
    for tool in svc.list_tools_view() {
        assert!(
            magistr_workflow::mcp::UnifiedFamily::Run.claims(&tool.name),
            "tool '{}' should be a run_*, approval_* or step verb",
            tool.name
        );
    }
}

/// `run_run` is the name a caller derives instead of reading `run_finish`.
/// It must never become real: a served `run_run` would make the misderivation
/// correct and the finalize verb ambiguous.
#[test]
fn the_misderived_finalize_name_is_not_served() {
    let svc = service();
    let names: Vec<String> = svc
        .list_tools_view()
        .iter()
        .map(|t| t.name.to_string())
        .collect();
    assert!(!names.contains(&"run_run".to_string()));
    assert!(names.contains(&"run_finish".to_string()));
}

#[test]
fn output_schema_is_valid_json_schema() {
    let svc = service();
    for tool in svc.list_tools_view() {
        let schema = tool.output_schema.as_ref().unwrap();
        jsonschema::validator_for(&serde_json::Value::Object((**schema).clone())).unwrap_or_else(
            |error| panic!("tool '{}' has an invalid output schema: {error}", tool.name),
        );
    }
}
