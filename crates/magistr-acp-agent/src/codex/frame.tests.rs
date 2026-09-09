use super::*;

#[test]
fn malformed_output_is_distinct_from_future_notifications() {
    assert!(matches!(Frame::parse("not json"), Frame::Malformed(_)));
    assert!(matches!(Frame::parse("[]"), Frame::Malformed(_)));
    assert!(matches!(
        Frame::parse(r#"{"id":null,"result":{}}"#),
        Frame::Malformed(_)
    ));
    assert!(matches!(
        Frame::parse(r#"{"method":"future/update","params":{}}"#),
        Frame::Notification { .. }
    ));
}

#[test]
fn numeric_zero_request_and_opaque_string_response_keep_their_ids() {
    assert!(
        matches!(Frame::parse(r#"{"id":0,"method":"approve","params":{"threadId":"thread"}}"#), Frame::Request { id, .. } if id == 0)
    );
    assert!(
        matches!(Frame::parse(r#"{"id":"opaque","result":{}}"#), Frame::Response { id, .. } if id == "opaque")
    );
}

#[test]
fn structured_errors_keep_codex_classification() {
    let error = RpcError::parse(
        &serde_json::json!({"code":-32000,"message":"quota exhausted","data":{"codexErrorInfo":"usageLimitExceeded"}}),
    );
    assert!(error.to_string().contains("usageLimitExceeded"));
    assert_eq!(
        RpcError::classification(
            &serde_json::json!({"httpConnectionFailed":{"httpStatusCode":502}})
        ),
        Some("httpConnectionFailed (HTTP 502)".into())
    );
}
