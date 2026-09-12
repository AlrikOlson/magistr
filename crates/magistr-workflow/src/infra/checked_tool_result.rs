//! Guard composed calls before reading success fields; errors retain the original result.
use rmcp::model::CallToolResult;
use serde_json::Value;

pub struct CheckedToolResult;

impl CheckedToolResult {
    pub fn body(result: &CallToolResult) -> Result<Value, Box<CallToolResult>> {
        if result.is_error == Some(true) {
            return Err(Box::new(result.clone()));
        }
        let body = result.structured_content.clone().or_else(|| {
            result.content.iter().find_map(|item| {
                item.as_text()
                    .and_then(|text| serde_json::from_str::<Value>(&text.text).ok())
            })
        });
        match body {
            Some(body) if body["ok"] == false || body["status"] == "failed" => {
                let mut error = result.clone();
                error.is_error = Some(true);
                Err(Box::new(error))
            }
            Some(body) if body.is_object() => Ok(body),
            _ => {
                let mut error = super::tool_failure::ToolFailure::uncertain(
                    "invalid_tool_result",
                    "Expected a structured object or JSON text; inspect the original response before retrying.",
                );
                // Original text and structured diagnostics survive even if they
                // could not be decoded. Never replace them with a field-access panic.
                error.content.extend(result.content.clone());
                if let Some(body) = error
                    .structured_content
                    .as_mut()
                    .and_then(Value::as_object_mut)
                {
                    body.insert(
                        "original_result".into(),
                        serde_json::to_value(result)
                            .expect("CallToolResult serialization is infallible"),
                    );
                }
                if let Some(body) = &error.structured_content {
                    error.content[0] = rmcp::model::ContentBlock::text(body.to_string());
                }
                Err(Box::new(error))
            }
        }
    }
}

#[cfg(test)]
#[path = "checked_tool_result.tests.rs"]
mod tests;
