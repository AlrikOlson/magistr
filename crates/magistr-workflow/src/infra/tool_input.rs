//! Required-field admission before rmcp decoding; persisted defaults are untouched.
use super::tool_failure::ToolFailure;
use rmcp::{
    ErrorData,
    model::{CallToolRequestParams, CallToolResponse, CallToolResult, Tool},
};
use serde_json::{Value, json};

pub struct ToolInput;

impl ToolInput {
    /// Return all absent required top-level fields. Domain validation still
    /// owns ranges, semantic prerequisites and supported compatibility coercions.
    pub fn required(tool: &Tool, request: &CallToolRequestParams) -> Option<CallToolResult> {
        let required = tool
            .input_schema
            .get("required")
            .and_then(Value::as_array)?;
        let mut problems = Vec::new();
        for field in required.iter().filter_map(Value::as_str) {
            let present = request.arguments.as_ref().is_some_and(|args| {
                args.contains_key(field)
                    || (field == "cross_ref"
                        && (args.contains_key("ref") || args.contains_key("crossref")))
            });
            if !present {
                problems.push(json!({"field":field,"message":"Required argument is missing; supply it as a separate top-level field."}));
            }
        }
        (!problems.is_empty()).then(|| {
            ToolFailure::rejected(
                "invalid_args",
                "Required tool arguments are missing.",
                problems,
            )
        })
    }

    /// Preserve protocol errors for unknown tools. A known tool's argument
    /// decoding error is actionable tool feedback and cannot have applied effects.
    pub fn decoded(
        result: Result<CallToolResponse, ErrorData>,
        known: bool,
    ) -> Result<CallToolResponse, ErrorData> {
        match result {
            Err(error) if known && error.code.0 == -32602 => {
                let problem = json!({"field":"$","message":error.message,"diagnostic":error.data});
                Ok(ToolFailure::rejected(
                    "invalid_args",
                    "Tool arguments could not be decoded; consult the original diagnostic.",
                    vec![problem],
                )
                .into())
            }
            other => other,
        }
    }
}
