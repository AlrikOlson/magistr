//! The control frames the provider writes to the CLI.

use serde_json::Value;

/// A `control_request` the provider sends. The CLI answers each with one
/// `control_response` carrying the same request id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ControlRequest {
    /// The handshake: answered with the model catalog, the command list and
    /// the account; sent once per process before anything else.
    Initialize,
    /// Abort the running turn; the turn then ends with an error result.
    Interrupt,
    /// Switch the model for the next turns; `None` resets to the default.
    SetModel(Option<String>),
}

impl ControlRequest {
    /// The frame, with `request_id` for the CLI to echo.
    pub fn frame(&self, request_id: &str) -> Value {
        let request = match self {
            Self::Initialize => serde_json::json!({"subtype": "initialize"}),
            Self::Interrupt => serde_json::json!({"subtype": "interrupt"}),
            Self::SetModel(model) => serde_json::json!({"subtype": "set_model", "model": model}),
        };
        serde_json::json!({
            "type": "control_request",
            "request_id": request_id,
            "request": request,
        })
    }

    /// The `control_response` frame answering a request the CLI sent:
    /// `Ok(payload)` is a success carrying it, `Err(message)` a refusal.
    pub fn response(request_id: &str, outcome: Result<Value, String>) -> Value {
        let response = match outcome {
            Ok(payload) => serde_json::json!({
                "subtype": "success",
                "request_id": request_id,
                "response": payload,
            }),
            Err(error) => serde_json::json!({
                "subtype": "error",
                "request_id": request_id,
                "error": error,
            }),
        };
        serde_json::json!({ "type": "control_response", "response": response })
    }

    /// The frame that carries one prompt turn: a user message of text blocks.
    pub fn user_message(text: &str) -> Value {
        serde_json::json!({
            "type": "user",
            "message": {"role": "user", "content": [{"type": "text", "text": text}]},
            "parent_tool_use_id": null,
            "session_id": "",
        })
    }
}

#[cfg(test)]
#[path = "control.tests.rs"]
mod tests;
