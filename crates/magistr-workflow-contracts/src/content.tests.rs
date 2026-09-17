use super::*;

fn minimal() -> StructuredContent {
    StructuredContent {
        version: CONTENT_VERSION,
        summary: "A plain sentence.".to_string(),
        facts: None,
        sections: None,
    }
}

#[test]
fn minimal_content_validates_and_round_trips() {
    let content = minimal();
    content.validate().unwrap();
    let wire = serde_json::to_value(&content).unwrap();
    assert_eq!(
        wire,
        serde_json::json!({ "version": 1, "summary": "A plain sentence." })
    );
    let back: StructuredContent = serde_json::from_value(wire).unwrap();
    assert_eq!(back, content);
}

#[test]
fn schema_example_shape_deserializes() {
    // The signal example in the canonical schema, record.content verbatim.
    let wire = serde_json::json!({
        "version": 1,
        "summary": "Exporting the roadmap loses nested acceptance notes under their chunks.",
        "facts": [
            { "label": "kind", "value": "bug" },
            { "label": "surface", "value": "roadmap export" }
        ],
        "sections": [
            { "heading": "What happens", "prose": "Running an export…" },
            { "heading": "How to reproduce", "list": [
                { "text": "Add a chunk with a two-level acceptance list" },
                { "text": "Export the roadmap and compare", "done": false }
            ] }
        ]
    });
    let content: StructuredContent = serde_json::from_value(wire.clone()).unwrap();
    content.validate().unwrap();
    assert_eq!(serde_json::to_value(&content).unwrap(), wire);
}

#[test]
fn parse_optional_accepts_a_json_encoded_string() {
    // What a schema-decoding client (Claude Code) sends for an untyped
    // field: the object serialized into one JSON string.
    let wire = serde_json::json!({ "version": 1, "summary": "A plain sentence." });
    let encoded = serde_json::Value::String(wire.to_string());
    let parsed = parse_optional(Some(encoded)).unwrap().unwrap();
    assert_eq!(parsed, minimal());
}

#[test]
fn parse_optional_blank_string_is_none() {
    assert_eq!(
        parse_optional(Some(serde_json::Value::String("  ".to_string()))).unwrap(),
        None
    );
}

#[test]
fn parse_optional_rejects_non_json_and_double_encoding_with_the_reason() {
    let err = parse_optional(Some(serde_json::Value::String("not json".to_string()))).unwrap_err();
    assert!(err.contains("not valid JSON"), "{err}");

    let doubled = serde_json::Value::String(
        serde_json::Value::String("{\"version\":1}".to_string()).to_string(),
    );
    let err = parse_optional(Some(doubled)).unwrap_err();
    assert!(err.contains("doubly"), "{err}");
}

#[test]
fn parse_optional_still_validates_a_decoded_string() {
    // The string decodes fine but the object inside violates the shape —
    // the validation error must be the same one a real object gets.
    let wire = serde_json::json!({ "version": 2, "summary": "A plain sentence." });
    let err = parse_optional(Some(serde_json::Value::String(wire.to_string()))).unwrap_err();
    assert!(err.contains("version"), "{err}");
}

#[test]
fn input_schema_declares_the_object_type() {
    let mut generator = schemars::SchemaGenerator::default();
    let schema = input_schema(&mut generator).to_value();
    assert_eq!(schema["type"], serde_json::json!(["object", "null"]));
}

#[test]
fn violations_name_the_field() {
    let mut c = minimal();
    c.summary = String::new();
    assert!(c.validate().unwrap_err().contains("summary"));

    let mut c = minimal();
    c.version = 2;
    assert!(c.validate().unwrap_err().contains("version"));

    let mut c = minimal();
    c.facts = Some(vec![ContentFact {
        label: "x".repeat(49),
        value: "v".to_string(),
    }]);
    assert!(c.validate().unwrap_err().contains("label"));
}
