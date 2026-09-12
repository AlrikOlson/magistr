//! Live call requirements are independent of persisted ThinkStep defaults.
use crate::think::domain::ThinkStep;
use rmcp::model::{CallToolResult, Tool};
use serde_json::{Map, Value, json};
use std::sync::Arc;

pub(super) struct RecordInput;

impl RecordInput {
    const PROSE: [&'static str; 5] = ["purpose", "context", "thought", "outcome", "rationale"];

    pub(super) fn publish(tool: &mut Tool) {
        let schema = Arc::make_mut(&mut tool.input_schema);
        let mut required: Vec<Value> = Self::PROSE.iter().map(|f| json!(f)).collect();
        required.extend([json!("estimated_total"), json!("next_action")]);
        schema.insert("required".into(), json!(required));
        if let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) {
            for field in Self::PROSE {
                properties.insert(field.into(), json!({"type":"string", "pattern":r"\S"}));
            }
            properties.insert(
                "estimated_total".into(),
                json!({"type":"integer", "minimum":1, "maximum":u32::MAX}),
            );
            properties.insert(
                "next_action".into(),
                json!({"anyOf":[
                    {"type":"string","pattern":r"\S"},
                    {"type":"object","required":["action"],"properties":{
                        "action":{"type":"string","pattern":r"\S"},
                        "tool":{"type":["string","null"]},
                        "parameters":{"type":["object","null"]},
                        "expectedOutput":{"type":["string","null"]}
                    }}
                ]}),
            );
        }
    }

    pub(super) fn validate(arguments: &Map<String, Value>) -> Option<CallToolResult> {
        let mut problems = Vec::new();
        for field in Self::PROSE {
            if arguments
                .get(field)
                .and_then(Value::as_str)
                .is_none_or(|s| s.trim().is_empty())
            {
                problems.push(json!({"field":field,"message":"Supply a nonblank string as a separate top-level argument."}));
            }
        }
        if !arguments
            .get("estimated_total")
            .and_then(Value::as_u64)
            .is_some_and(|n| n > 0 && n <= u64::from(u32::MAX))
        {
            problems.push(json!({"field":"estimated_total","message":"Supply an integer from 1 through 4294967295."}));
        }
        let action = arguments.get("next_action");
        let (field, value) = if action.is_some_and(Value::is_object) {
            ("next_action.action", action.and_then(|v| v.get("action")))
        } else {
            ("next_action", action)
        };
        if value
            .and_then(Value::as_str)
            .is_none_or(|s| s.trim().is_empty())
        {
            problems.push(json!({"field":field,"message":"Supply a nonblank string or an object with a nonblank action."}));
        }
        // Deserialize each supplied field in isolation to retain its name when
        // serde rejects optional/nested types. This never produces stored text.
        for (field, value) in arguments {
            let single = json!({field: value});
            if let Err(error) = serde_json::from_value::<ThinkStep>(single) {
                problems.push(json!({"field":field,"message":error.to_string()}));
            }
        }
        if problems.is_empty() {
            return None;
        }
        Some(crate::infra::tool_failure::ToolFailure::rejected(
            "invalid_args",
            "Reasoning input is invalid; no step was recorded. Supply each authored field separately; never infer missing reasoning.",
            problems,
        ))
    }
}
