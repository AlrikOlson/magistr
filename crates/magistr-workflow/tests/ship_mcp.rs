use magistr_workflow::ship::engine::ShipEngine;
use magistr_workflow::ship::mcp::ShipService;

fn service() -> ShipService {
    let engine = ShipEngine::new("test-abc123".into());
    ShipService::new(engine)
}

#[test]
fn tools_list_has_14_canonical_entries() {
    let svc = service();
    let tools = svc.list_tools_view();
    assert_eq!(
        tools.len(),
        14,
        "expected 14 ship_* tools, including explicit receipt compaction, got {}",
        tools.len()
    );
    let ship_count = tools.iter().filter(|t| t.name.starts_with("ship_")).count();
    assert_eq!(ship_count, 14);
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
    let destructive_names = ["ship_reset", "ship_compact_receipts"];
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
        "ship_status",
        "ship_export",
        // A gate poll reads the workspace and writes nothing.
        "ship_gate_wait",
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
fn every_tool_name_is_a_ship_canonical() {
    let svc = service();
    for tool in svc.list_tools_view() {
        assert!(
            tool.name.starts_with("ship_"),
            "tool '{}' should start with 'ship_'",
            tool.name
        );
    }
}

/// `ship_ship` is the name a caller derives instead of reading `ship_finalize`.
/// It must never become real: a served `ship_ship` would make the misderivation
/// correct and the finalize verb ambiguous.
#[test]
fn the_misderived_finalize_name_is_not_served() {
    let svc = service();
    let names: Vec<String> = svc
        .list_tools_view()
        .iter()
        .map(|t| t.name.to_string())
        .collect();
    assert!(!names.contains(&"ship_ship".to_string()));
    assert!(names.contains(&"ship_finalize".to_string()));
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
