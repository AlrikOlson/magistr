use super::*;

use std::time::Duration;

#[test]
fn connection_lost_display() {
    let err = EngineClientError::ConnectionLost("pipe broken".into());
    assert_eq!(err.to_string(), "connection lost: pipe broken");
}

#[test]
fn timeout_display() {
    let err = EngineClientError::Timeout {
        operation: "send_command".into(),
        duration: Duration::from_secs(30),
    };
    let display = err.to_string();
    assert!(display.contains("send_command"));
    assert!(display.contains("timed out"));
    assert!(display.contains("30"));
}

#[test]
fn protocol_display() {
    let err = EngineClientError::Protocol("invalid JSON-RPC version".into());
    assert_eq!(err.to_string(), "protocol error: invalid JSON-RPC version");
}

#[test]
fn invalid_state_display() {
    let err = EngineClientError::InvalidState {
        operation: "send_command".into(),
        state: "Completed".into(),
    };
    assert_eq!(
        err.to_string(),
        "invalid state for send_command: session is Completed"
    );
}

#[test]
fn from_magistr_error() {
    let magistr_err = crate::error::MagistrError::Other("test error".into());
    let engine_err: EngineClientError = magistr_err.into();
    assert!(matches!(engine_err, EngineClientError::Agent(_)));
    assert!(engine_err.to_string().contains("test error"));
}

#[test]
fn agent_error_source() {
    use std::error::Error;

    let magistr_err = crate::error::MagistrError::Other("inner".into());
    let engine_err = EngineClientError::Agent(magistr_err);
    assert!(engine_err.source().is_some());
}

#[test]
fn non_agent_errors_have_no_source() {
    use std::error::Error;

    let err = EngineClientError::ConnectionLost("gone".into());
    assert!(err.source().is_none());

    let err = EngineClientError::Protocol("bad".into());
    assert!(err.source().is_none());
}
