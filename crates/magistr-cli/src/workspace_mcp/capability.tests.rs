use super::*;

fn version(text: &str) -> ProtocolVersion {
    serde_json::from_value(Value::String(text.to_owned())).expect("version")
}

#[test]
fn a_served_era_is_echoed_and_everything_else_gets_the_default_era() {
    for served in ["2024-11-05", "2025-03-26", "2025-06-18"] {
        assert_eq!(
            Capability::initialize(&version(served)).protocol_version,
            version(served),
            "{served}"
        );
    }
    for other in ["2025-11-25", "2026-07-28", "2027-01-01", "1999-01-01"] {
        assert_eq!(
            Capability::initialize(&version(other)).protocol_version,
            ProtocolVersion::V_2025_06_18,
            "{other}"
        );
    }
}

#[test]
fn the_handshake_declares_tools_tasks_instructions_and_this_binary() {
    let result = serde_json::to_value(Capability::initialize(&version("2025-06-18")))
        .expect("initialize result");
    assert!(result["capabilities"]["tools"].is_object(), "{result}");
    assert!(
        result["capabilities"]["extensions"]["io.modelcontextprotocol/tasks"].is_object(),
        "SEP-2663 tasks ride the extensions map: {result}"
    );
    assert_eq!(result["serverInfo"]["name"], "magistr");
    assert_eq!(result["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(result["instructions"], tool_surface::INSTRUCTIONS);
    assert_eq!(result["protocolVersion"], "2025-06-18");
    assert_eq!(
        Capability::supported_protocol_versions().as_ref(),
        SERVED_PROTOCOL_VERSIONS
    );
}

#[test]
fn the_core_listing_is_the_served_wire_shape_with_or_without_script() {
    let names = |listing: Value| -> Vec<String> {
        assert!(listing.get("resultType").is_none(), "{listing}");
        assert!(listing.get("nextCursor").is_none(), "{listing}");
        listing["tools"]
            .as_array()
            .expect("tools")
            .iter()
            .map(|tool| tool["name"].as_str().expect("name").to_owned())
            .collect()
    };
    assert_eq!(
        names(Capability::new(true).core_listing()),
        [
            "workbench_status",
            "map",
            "read",
            "search",
            "edit",
            "files",
            "write",
            "undo",
            "script",
            "run"
        ]
    );
    assert_eq!(
        names(Capability::new(false).core_listing()),
        [
            "workbench_status",
            "map",
            "read",
            "search",
            "edit",
            "files",
            "write",
            "undo",
            "run"
        ]
    );
    let listing = Capability::new(true).core_listing();
    let edit = &listing["tools"][4];
    assert_eq!(edit["title"], "Edit the workspace");
    assert!(edit["inputSchema"].is_object(), "{edit}");
    assert!(edit["outputSchema"].is_object(), "{edit}");
}
