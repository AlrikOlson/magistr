use super::*;
use agent_client_protocol::ErrorCode;

#[test]
fn a_provider_failure_is_an_internal_error_carrying_the_message() {
    let wire: agent_client_protocol::Error = AgentError::provider("model went away").into();
    assert_eq!(wire.code, ErrorCode::InternalError);
    assert_eq!(wire.data, Some(serde_json::json!("model went away")));
}

#[test]
fn an_unknown_session_is_invalid_params_naming_the_session() {
    let wire: agent_client_protocol::Error = AgentError::UnknownSession("sess-9".into()).into();
    assert_eq!(wire.code, ErrorCode::InvalidParams);
    assert_eq!(
        wire.data,
        Some(serde_json::json!("unknown session: sess-9"))
    );
}

#[test]
fn a_protocol_error_passes_through_unchanged() {
    let original = agent_client_protocol::Error::method_not_found();
    let wire: agent_client_protocol::Error = AgentError::Protocol(original.clone()).into();
    assert_eq!(wire.code, original.code);
}
