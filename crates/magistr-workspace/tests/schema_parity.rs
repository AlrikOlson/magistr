//! Native tool schemas describe the current product contract. Historical
//! reference bytes do not constrain changes to this unreleased API.

use magistr_workspace::service::ToolSchemas;

#[test]
fn every_tool_schema_describes_an_object_with_its_tool_name() {
    for tool in ToolSchemas::TOOL_NAMES {
        let parsed = ToolSchemas::input_value(tool).expect("valid JSON");
        assert_eq!(parsed["title"], tool);
        assert_eq!(parsed["type"], "object");
    }
    assert_eq!(ToolSchemas::input("workbench_status"), None);
    assert_eq!(ToolSchemas::input("nope"), None);
}

#[test]
fn empty_file_population_is_advertised_by_the_current_tool_contracts() {
    let edit = ToolSchemas::input_value("edit").expect("edit schema");
    assert_eq!(edit["properties"]["old"]["type"], "string");
    assert!(edit["properties"]["old"].get("minLength").is_none());
    let description = edit["properties"]["old"]["description"].as_str().unwrap();
    assert!(description.contains("zero-byte") && description.contains("STALE_ANCHOR"));
    let files = ToolSchemas::input_value("files").expect("files schema");
    assert!(
        files["properties"]["op"]["description"]
            .as_str()
            .unwrap()
            .contains("Populate")
    );
    let script = ToolSchemas::input_value("script").expect("script schema");
    let description = script["description"].as_str().unwrap();
    assert!(description.contains("expectedHash") && description.contains("zero-byte"));
}

#[test]
fn the_envelope_schema_requires_version_and_outcome() {
    let envelope = ToolSchemas::envelope_value();
    assert_eq!(envelope["title"], "ResponseEnvelope");
    assert_eq!(envelope["required"], serde_json::json!(["v", "ok"]));
}
