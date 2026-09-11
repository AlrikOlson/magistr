//! The native tool schemas are the reference's, byte for byte: the served
//! `inputSchema` of every tool and the envelope `outputSchema` never drift
//! from the pinned tree without this test saying so.
use std::path::PathBuf;

use magistr_workspace::service::ToolSchemas;

fn reference_schemas() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../subsystems/workspace/reference/schemas")
}

#[test]
fn every_tool_schema_is_byte_identical_to_the_reference() {
    let reference = reference_schemas().join("tools");
    for tool in ToolSchemas::TOOL_NAMES {
        let pinned = std::fs::read_to_string(reference.join(format!("{tool}.input.schema.json")))
            .expect("pinned reference schema");
        assert_eq!(
            ToolSchemas::input(tool),
            Some(pinned.as_str()),
            "{tool}: native schema bytes differ from the reference"
        );
        let parsed = ToolSchemas::input_value(tool).expect("valid JSON");
        assert_eq!(parsed["title"], tool);
        assert_eq!(parsed["type"], "object");
    }
    assert_eq!(ToolSchemas::input("workbench_status"), None);
    assert_eq!(ToolSchemas::input("nope"), None);
}

#[test]
fn the_envelope_schema_is_byte_identical_to_the_reference() {
    let pinned = std::fs::read_to_string(reference_schemas().join("envelope.schema.json"))
        .expect("pinned envelope schema");
    assert_eq!(ToolSchemas::ENVELOPE, pinned);
    let envelope = ToolSchemas::envelope_value();
    assert_eq!(envelope["title"], "ResponseEnvelope");
    assert_eq!(envelope["required"], serde_json::json!(["v", "ok"]));
}
