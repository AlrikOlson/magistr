//! Protocol-level error codes and types.
//!
//! Defines the standard JSON-RPC 2.0 error codes plus magistr-specific
//! application error codes for protocol-level failures.

use serde::{Deserialize, Serialize};

// ── Standard JSON-RPC 2.0 error codes ──────────────────────────────────────

/// Invalid JSON was received by the server.
pub const PARSE_ERROR: i64 = -32700;

/// The JSON sent is not a valid Request object.
pub const INVALID_REQUEST: i64 = -32600;

/// The method does not exist or is not available.
pub const METHOD_NOT_FOUND: i64 = -32601;

/// Invalid method parameter(s).
pub const INVALID_PARAMS: i64 = -32602;

/// Internal JSON-RPC error.
pub const INTERNAL_ERROR: i64 = -32603;

// ── magistr application error codes (range: -32000 to -32099) ──────────────────

/// The requested session does not exist.
pub const SESSION_NOT_FOUND: i64 = -32000;

/// The command is invalid for the current session state.
pub const INVALID_COMMAND: i64 = -32001;

/// The workflow phase transition is invalid.
pub const INVALID_PHASE_TRANSITION: i64 = -32002;

/// A required precondition was not met.
pub const PRECONDITION_FAILED: i64 = -32003;

/// The budget limit has been exceeded.
pub const BUDGET_EXCEEDED: i64 = -32004;

/// The server is shutting down.
pub const SERVER_SHUTTING_DOWN: i64 = -32005;

/// Protocol version mismatch.
pub const VERSION_MISMATCH: i64 = -32006;

/// A protocol-level error with structured data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProtocolError {
    /// Error code (negative integer).
    pub code: i64,
    /// Human-readable error message.
    pub message: String,
    /// Optional structured error data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl ProtocolError {
    /// Create an error with just code and message.
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// Create an error with additional data.
    pub fn with_data(code: i64, message: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            code,
            message: message.into(),
            data: Some(data),
        }
    }

    /// Parse error.
    pub fn parse_error(detail: impl Into<String>) -> Self {
        Self::new(PARSE_ERROR, detail)
    }

    /// Invalid request.
    pub fn invalid_request(detail: impl Into<String>) -> Self {
        Self::new(INVALID_REQUEST, detail)
    }

    /// Method not found.
    pub fn method_not_found(method: &str) -> Self {
        Self::new(METHOD_NOT_FOUND, format!("method not found: {method}"))
    }

    /// Invalid params.
    pub fn invalid_params(detail: impl Into<String>) -> Self {
        Self::new(INVALID_PARAMS, detail)
    }

    /// Internal error.
    pub fn internal(detail: impl Into<String>) -> Self {
        Self::new(INTERNAL_ERROR, detail)
    }

    /// Session not found.
    pub fn session_not_found(session_id: &str) -> Self {
        Self::new(
            SESSION_NOT_FOUND,
            format!("session not found: {session_id}"),
        )
    }

    /// Invalid command for current state.
    pub fn invalid_command(detail: impl Into<String>) -> Self {
        Self::new(INVALID_COMMAND, detail)
    }

    /// Invalid phase transition.
    pub fn invalid_phase_transition(from: &str, to: &str) -> Self {
        Self::new(
            INVALID_PHASE_TRANSITION,
            format!("invalid phase transition from {from} to {to}"),
        )
    }

    /// Budget exceeded.
    pub fn budget_exceeded(detail: impl Into<String>) -> Self {
        Self::new(BUDGET_EXCEEDED, detail)
    }
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for ProtocolError {}

/// Convert an `EngineError` into a `ProtocolError`.
impl From<magistr_engine::EngineError> for ProtocolError {
    fn from(err: magistr_engine::EngineError) -> Self {
        match &err {
            magistr_engine::EngineError::InvalidCommand { .. } => {
                Self::new(INVALID_COMMAND, err.to_string())
            }
            magistr_engine::EngineError::InvalidPhaseTransition { .. } => {
                Self::new(INVALID_PHASE_TRANSITION, err.to_string())
            }
            magistr_engine::EngineError::Precondition(_) => {
                Self::new(PRECONDITION_FAILED, err.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_error_codes() {
        assert_eq!(PARSE_ERROR, -32700);
        assert_eq!(INVALID_REQUEST, -32600);
        assert_eq!(METHOD_NOT_FOUND, -32601);
        assert_eq!(INVALID_PARAMS, -32602);
        assert_eq!(INTERNAL_ERROR, -32603);
    }

    #[test]
    fn application_error_codes_in_range() {
        let codes = [
            SESSION_NOT_FOUND,
            INVALID_COMMAND,
            INVALID_PHASE_TRANSITION,
            PRECONDITION_FAILED,
            BUDGET_EXCEEDED,
            SERVER_SHUTTING_DOWN,
            VERSION_MISMATCH,
        ];
        for code in codes {
            assert!(
                (-32099..=-32000).contains(&code),
                "code {code} not in application range"
            );
        }
    }

    #[test]
    fn protocol_error_display() {
        let err = ProtocolError::new(PARSE_ERROR, "unexpected token");
        assert_eq!(err.to_string(), "[-32700] unexpected token");
    }

    #[test]
    fn protocol_error_serialization_roundtrip() {
        let err = ProtocolError::with_data(
            INVALID_PARAMS,
            "missing field: session_id",
            serde_json::json!({"field": "session_id"}),
        );
        let json = serde_json::to_string(&err).unwrap();
        let deserialized: ProtocolError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, deserialized);
    }

    #[test]
    fn protocol_error_without_data_skips_field() {
        let err = ProtocolError::new(INTERNAL_ERROR, "boom");
        let json = serde_json::to_value(&err).unwrap();
        assert!(json.get("data").is_none());
    }

    #[test]
    fn from_engine_error_invalid_command() {
        let engine_err =
            magistr_engine::EngineError::invalid_command("Cancel", "Completed", "session ended");
        let proto_err = ProtocolError::from(engine_err);
        assert_eq!(proto_err.code, INVALID_COMMAND);
        assert!(proto_err.message.contains("Cancel"));
    }

    #[test]
    fn from_engine_error_invalid_phase() {
        let engine_err =
            magistr_engine::EngineError::invalid_phase_transition("Verifying", "Researching");
        let proto_err = ProtocolError::from(engine_err);
        assert_eq!(proto_err.code, INVALID_PHASE_TRANSITION);
    }

    #[test]
    fn from_engine_error_precondition() {
        let engine_err = magistr_engine::EngineError::Precondition("missing config".into());
        let proto_err = ProtocolError::from(engine_err);
        assert_eq!(proto_err.code, PRECONDITION_FAILED);
    }

    #[test]
    fn convenience_constructors() {
        assert_eq!(ProtocolError::parse_error("bad json").code, PARSE_ERROR);
        assert_eq!(
            ProtocolError::method_not_found("foo.bar").code,
            METHOD_NOT_FOUND
        );
        assert_eq!(
            ProtocolError::session_not_found("abc").code,
            SESSION_NOT_FOUND
        );
        assert_eq!(
            ProtocolError::budget_exceeded("over limit").code,
            BUDGET_EXCEEDED
        );
    }
}
