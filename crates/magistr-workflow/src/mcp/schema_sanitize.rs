//! Keep served schemas portable and output validation compact.
//!
//! schemars stamps Rust integer widths as `format: "uint32"` / `"uint64"`
//! (and friends) on generated schemas. Those aren't JSON Schema formats, so
//! MCP clients (`claude mcp list` among them) warn `unknown format … ignored`
//! on every connect. This is the one seam: every family's `list_tools_view`
//! runs its tools through [`sanitize_tool_schemas`] before serving, dropping
//! the unrecognized hints while leaving types, bounds, and standard formats
//! (date-time, uri, uuid, …) untouched.

use rmcp::model::Tool;
use serde_json::Value;
use std::sync::Arc;

/// Formats defined by JSON Schema 2020-12 (the set clients recognize).
const STANDARD_FORMATS: &[&str] = &[
    "date-time",
    "date",
    "time",
    "duration",
    "email",
    "idn-email",
    "hostname",
    "idn-hostname",
    "ipv4",
    "ipv6",
    "uri",
    "uri-reference",
    "iri",
    "iri-reference",
    "uuid",
    "json-pointer",
    "relative-json-pointer",
    "regex",
];

/// Recursively drop any `format` whose value isn't a standard JSON Schema
/// format. Everything else (types, `minimum`, `required`, …) is untouched.
fn sanitize_value(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let drop = matches!(
                map.get("format"),
                Some(Value::String(f)) if !STANDARD_FORMATS.contains(&f.as_str())
            );
            if drop {
                map.remove("format");
            }
            for v in map.values_mut() {
                sanitize_value(v);
            }
        }
        Value::Array(items) => {
            for v in items.iter_mut() {
                sanitize_value(v);
            }
        }
        _ => {}
    }
}

/// Sanitize a tool's input and output schemas in place (rebuilding the Arcs).
pub fn sanitize_tool_schemas(tool: &mut Tool) {
    let mut input = Value::Object((*tool.input_schema).clone());
    sanitize_value(&mut input);
    if let Value::Object(obj) = input {
        tool.input_schema = Arc::new(obj);
    }
    if let Some(schema) = tool.output_schema.take() {
        let mut output = Value::Object((*schema).clone());
        sanitize_value(&mut output);
        compact_output_schema(&mut output);
        if let Value::Object(obj) = output {
            tool.output_schema = Some(Arc::new(obj));
        }
    }
}

/// Output descriptions repeat tool and domain documentation on every response schema.
/// Keep validation intact, including properties named "title" or "description" and
/// literal values under const/default/examples; only visit schema-bearing keywords.
fn compact_output_schema(schema: &mut Value) {
    let Some(map) = schema.as_object_mut() else {
        return;
    };
    map.remove("title");
    map.remove("description");
    for key in [
        "$defs",
        "definitions",
        "properties",
        "patternProperties",
        "dependentSchemas",
    ] {
        if let Some(Value::Object(children)) = map.get_mut(key) {
            for child in children.values_mut() {
                compact_output_schema(child);
            }
        }
    }
    for key in [
        "items",
        "contains",
        "additionalProperties",
        "unevaluatedProperties",
        "propertyNames",
        "not",
        "if",
        "then",
        "else",
    ] {
        if let Some(child) = map.get_mut(key) {
            compact_output_schema(child);
        }
    }
    for key in ["allOf", "anyOf", "oneOf", "prefixItems"] {
        if let Some(Value::Array(children)) = map.get_mut(key) {
            for child in children {
                compact_output_schema(child);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn strips_nonstandard_formats_at_any_depth() {
        let mut v = json!({
            "type": "object",
            "properties": {
                "step_number": { "type": "integer", "format": "uint32", "minimum": 0 },
                "nested": {
                    "items": [{ "type": "integer", "format": "uint64" }],
                    "anyOf": [{ "type": "number", "format": "double" }]
                }
            }
        });
        sanitize_value(&mut v);
        let s = v.to_string();
        assert!(!s.contains("uint32") && !s.contains("uint64") && !s.contains("double"));
        // Bounds survive the cleanup.
        assert_eq!(v["properties"]["step_number"]["minimum"], json!(0));
    }

    #[test]
    fn compact_output_retains_the_validation_contract_and_literal_fields() {
        let original = json!({
            "type": "object", "description": "Repeated domain documentation",
            "required": ["description", "title"],
            "properties": {
                "description": { "type": "string", "description": "Field documentation", "minLength": 3 },
                "title": { "const": { "description": "literal", "title": "literal" } }
            },
            "additionalProperties": false
        });
        let mut compact = original.clone();
        compact_output_schema(&mut compact);
        let before = jsonschema::validator_for(&original).unwrap();
        let after = jsonschema::validator_for(&compact).unwrap();
        let valid =
            json!({"description": "yes", "title": {"description": "literal", "title": "literal"}});
        for (value, expected) in [
            (valid, true),
            (json!({}), false),
            (json!({"description": "no", "title": {}}), false),
        ] {
            assert_eq!(before.is_valid(&value), expected);
            assert_eq!(after.is_valid(&value), expected);
        }
        assert!(compact.to_string().len() < original.to_string().len());
    }

    #[test]
    fn keeps_standard_formats() {
        let mut v = json!({ "created": { "type": "string", "format": "date-time" } });
        sanitize_value(&mut v);
        assert_eq!(v["created"]["format"], json!("date-time"));
    }
}
