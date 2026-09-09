//! JSON-RPC 2.0 base types.
//!
//! Defines the wire-format types for JSON-RPC 2.0 requests, responses,
//! and notifications. These are protocol-agnostic — any transport
//! (stdio, socket, WebSocket) serializes/deserializes these types.

use serde::{Deserialize, Serialize};

use crate::error::ProtocolError;

/// JSON-RPC 2.0 version string.
pub const JSONRPC_VERSION: &str = "2.0";

// ── Request ID ──────────────────────────────────────────────────────────────

/// JSON-RPC request identifier.
///
/// Per the spec, `id` can be a string, number, or null.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    /// Numeric ID.
    Number(i64),
    /// String ID.
    String(String),
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Number(n) => write!(f, "{n}"),
            Self::String(s) => write!(f, "{s}"),
        }
    }
}

impl From<i64> for RequestId {
    fn from(n: i64) -> Self {
        Self::Number(n)
    }
}

impl From<String> for RequestId {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<&str> for RequestId {
    fn from(s: &str) -> Self {
        Self::String(s.to_owned())
    }
}

// ── Request ─────────────────────────────────────────────────────────────────

/// A JSON-RPC 2.0 request.
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "session.status", "params": {...}}
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    /// Must be "2.0".
    pub jsonrpc: String,
    /// Request identifier.
    pub id: RequestId,
    /// Method name (e.g., "session.create", "workflow.phase").
    pub method: String,
    /// Method parameters. Omitted or null if the method takes no params.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl Request {
    /// Create a new request.
    pub fn new(
        id: impl Into<RequestId>,
        method: impl Into<String>,
        params: Option<serde_json::Value>,
    ) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            id: id.into(),
            method: method.into(),
            params,
        }
    }
}

// ── Response ────────────────────────────────────────────────────────────────

/// A JSON-RPC 2.0 response (success).
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {...}}
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    /// Must be "2.0".
    pub jsonrpc: String,
    /// Matching request identifier.
    pub id: RequestId,
    /// The result value.
    pub result: serde_json::Value,
}

impl Response {
    /// Create a success response.
    pub fn success(id: RequestId, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            id,
            result,
        }
    }
}

// ── Error response ──────────────────────────────────────────────────────────

/// A JSON-RPC 2.0 error response.
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "error": {"code": -32601, "message": "..."}}
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Must be "2.0".
    pub jsonrpc: String,
    /// Matching request identifier (may be null if request couldn't be parsed).
    pub id: Option<RequestId>,
    /// Error details.
    pub error: ProtocolError,
}

impl ErrorResponse {
    /// Create an error response.
    pub fn new(id: Option<RequestId>, error: ProtocolError) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            id,
            error,
        }
    }

    /// Create an error response matching a request ID.
    pub fn for_request(id: RequestId, error: ProtocolError) -> Self {
        Self::new(Some(id), error)
    }
}

// ── Notification ────────────────────────────────────────────────────────────

/// A JSON-RPC 2.0 notification (no `id` — no response expected).
///
/// ```json
/// {"jsonrpc": "2.0", "method": "session.event", "params": {...}}
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Notification {
    /// Must be "2.0".
    pub jsonrpc: String,
    /// Notification method name.
    pub method: String,
    /// Notification parameters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl Notification {
    /// Create a new notification.
    pub fn new(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            method: method.into(),
            params,
        }
    }
}

// ── Message (union type for parsing) ────────────────────────────────────────

/// A JSON-RPC 2.0 message — either a request, response, error, or notification.
///
/// This enum is used for parsing incoming messages when the type is unknown.
/// The parser checks for the presence of `id`, `method`, `result`, and `error`
/// fields to determine the variant.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// A request (has `id` and `method`).
    Request(Request),
    /// A success response (has `id` and `result`).
    Response(Response),
    /// An error response (has `error`).
    Error(ErrorResponse),
    /// A notification (has `method` but no `id`).
    Notification(Notification),
}

impl Message {
    /// Parse a JSON value into a JSON-RPC message.
    ///
    /// Determines the message type by inspecting which fields are present:
    /// - `method` + `id` → Request
    /// - `method` without `id` → Notification
    /// - `result` + `id` → Response
    /// - `error` → ErrorResponse
    pub fn from_value(value: serde_json::Value) -> Result<Self, ProtocolError> {
        let obj = value
            .as_object()
            .ok_or_else(|| ProtocolError::invalid_request("expected JSON object"))?;

        let has_id = obj.contains_key("id");
        let has_method = obj.contains_key("method");
        let has_result = obj.contains_key("result");
        let has_error = obj.contains_key("error");

        if has_method && has_id {
            serde_json::from_value(serde_json::Value::Object(obj.clone()))
                .map(Message::Request)
                .map_err(|e| ProtocolError::parse_error(e.to_string()))
        } else if has_method && !has_id {
            serde_json::from_value(serde_json::Value::Object(obj.clone()))
                .map(Message::Notification)
                .map_err(|e| ProtocolError::parse_error(e.to_string()))
        } else if has_result && has_id {
            serde_json::from_value(serde_json::Value::Object(obj.clone()))
                .map(Message::Response)
                .map_err(|e| ProtocolError::parse_error(e.to_string()))
        } else if has_error {
            serde_json::from_value(serde_json::Value::Object(obj.clone()))
                .map(Message::Error)
                .map_err(|e| ProtocolError::parse_error(e.to_string()))
        } else {
            Err(ProtocolError::invalid_request(
                "cannot determine message type: missing required fields",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── RequestId ───────────────────────────────────────────────────────

    #[test]
    fn request_id_number() {
        let id = RequestId::from(42_i64);
        assert_eq!(id.to_string(), "42");
        let json = serde_json::to_value(&id).unwrap();
        assert_eq!(json, serde_json::json!(42));
    }

    #[test]
    fn request_id_string() {
        let id = RequestId::from("abc-123");
        assert_eq!(id.to_string(), "abc-123");
        let json = serde_json::to_value(&id).unwrap();
        assert_eq!(json, serde_json::json!("abc-123"));
    }

    #[test]
    fn request_id_roundtrip() {
        let id = RequestId::Number(7);
        let json = serde_json::to_string(&id).unwrap();
        let parsed: RequestId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }

    // ── Request ─────────────────────────────────────────────────────────

    #[test]
    fn request_with_params() {
        let req = Request::new(
            1_i64,
            "session.create",
            Some(serde_json::json!({"name": "test"})),
        );
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["id"], 1);
        assert_eq!(json["method"], "session.create");
        assert_eq!(json["params"]["name"], "test");
    }

    #[test]
    fn request_without_params() {
        let req = Request::new(2_i64, "system.version", None);
        let json = serde_json::to_value(&req).unwrap();
        assert!(json.get("params").is_none());
    }

    #[test]
    fn request_serialization_roundtrip() {
        let req = Request::new("req-1", "workflow.phase", Some(serde_json::json!({})));
        let json = serde_json::to_string(&req).unwrap();
        let parsed: Request = serde_json::from_str(&json).unwrap();
        assert_eq!(req, parsed);
    }

    // ── Response ────────────────────────────────────────────────────────

    #[test]
    fn response_success() {
        let resp = Response::success(RequestId::from(1_i64), serde_json::json!({"status": "ok"}));
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["id"], 1);
        assert_eq!(json["result"]["status"], "ok");
    }

    #[test]
    fn response_serialization_roundtrip() {
        let resp = Response::success(RequestId::from(5_i64), serde_json::json!("done"));
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: Response = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, parsed);
    }

    // ── ErrorResponse ───────────────────────────────────────────────────

    #[test]
    fn error_response() {
        let err = ErrorResponse::for_request(
            RequestId::from(3_i64),
            ProtocolError::method_not_found("foo.bar"),
        );
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["error"]["code"], -32601);
        assert!(
            json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("foo.bar")
        );
    }

    #[test]
    fn error_response_null_id() {
        let err = ErrorResponse::new(None, ProtocolError::parse_error("bad json"));
        let json = serde_json::to_value(&err).unwrap();
        assert!(json["id"].is_null());
    }

    // ── Notification ────────────────────────────────────────────────────

    #[test]
    fn notification_with_params() {
        let notif = Notification::new("session.event", Some(serde_json::json!({"type": "ended"})));
        let json = serde_json::to_value(&notif).unwrap();
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["method"], "session.event");
        assert!(json.get("id").is_none());
    }

    #[test]
    fn notification_without_params() {
        let notif = Notification::new("system.shutdown", None);
        let json = serde_json::to_value(&notif).unwrap();
        assert!(json.get("params").is_none());
    }

    // ── Message parsing ─────────────────────────────────────────────────

    #[test]
    fn parse_request_message() {
        let json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "session.create",
            "params": {"name": "test"}
        });
        let msg = Message::from_value(json).unwrap();
        assert!(matches!(msg, Message::Request(_)));
    }

    #[test]
    fn parse_notification_message() {
        let json = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "session.event",
            "params": {"type": "ended"}
        });
        let msg = Message::from_value(json).unwrap();
        assert!(matches!(msg, Message::Notification(_)));
    }

    #[test]
    fn parse_response_message() {
        let json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {"status": "ok"}
        });
        let msg = Message::from_value(json).unwrap();
        assert!(matches!(msg, Message::Response(_)));
    }

    #[test]
    fn parse_error_message() {
        let json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {"code": -32601, "message": "not found"}
        });
        let msg = Message::from_value(json).unwrap();
        assert!(matches!(msg, Message::Error(_)));
    }

    #[test]
    fn parse_invalid_message() {
        let json = serde_json::json!({"jsonrpc": "2.0"});
        let err = Message::from_value(json).unwrap_err();
        assert_eq!(err.code, crate::error::INVALID_REQUEST);
    }

    #[test]
    fn parse_non_object_fails() {
        let json = serde_json::json!("hello");
        let err = Message::from_value(json).unwrap_err();
        assert_eq!(err.code, crate::error::INVALID_REQUEST);
    }
}
