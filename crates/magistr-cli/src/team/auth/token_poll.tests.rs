use super::*;

fn poll(text: &str) -> Result<TokenPoll, MagistrError> {
    TokenPoll::from_json(&serde_json::from_str(text).expect("valid json"))
}

#[test]
fn the_four_spec_errors_map_to_their_variants() {
    assert_eq!(
        poll(r#"{"error":"authorization_pending"}"#).expect("parse"),
        TokenPoll::Pending
    );
    assert_eq!(
        poll(r#"{"error":"slow_down"}"#).expect("parse"),
        TokenPoll::SlowDown
    );
    assert_eq!(
        poll(r#"{"error":"access_denied"}"#).expect("parse"),
        TokenPoll::Denied
    );
    assert_eq!(
        poll(r#"{"error":"expired_token"}"#).expect("parse"),
        TokenPoll::Expired
    );
}

#[test]
fn an_unknown_error_is_a_hard_failure_not_a_retry() {
    let err = poll(r#"{"error":"invalid_client","error_description":"bad id"}"#)
        .expect_err("should refuse");
    let text = err.to_string();
    assert!(text.contains("invalid_client"), "{text}");
    assert!(text.contains("bad id"), "{text}");
}

#[test]
fn an_unknown_error_without_a_description_still_names_itself() {
    let err = poll(r#"{"error":"unsupported_grant_type"}"#).expect_err("should refuse");
    assert!(err.to_string().contains("unsupported_grant_type"));
}

#[test]
fn a_granted_token_carries_the_id_token_when_present() {
    let TokenPoll::Granted(token) =
        poll(r#"{"access_token":"at","id_token":"header.payload.sig","expires_in":3600}"#)
            .expect("parse")
    else {
        panic!("expected a grant");
    };
    assert_eq!(token.access_token, "at");
    assert_eq!(token.id_token.as_deref(), Some("header.payload.sig"));
    assert_eq!(token.expires_in, Some(3600));
}

#[test]
fn github_style_grant_without_an_id_token_is_still_a_grant() {
    let TokenPoll::Granted(token) =
        poll(r#"{"access_token":"gho_x","token_type":"bearer","scope":"read:user"}"#)
            .expect("parse")
    else {
        panic!("expected a grant");
    };
    assert_eq!(token.id_token, None);
    assert_eq!(token.expires_in, None);
}

#[test]
fn a_body_with_neither_token_nor_error_is_refused() {
    let err = poll(r#"{"token_type":"bearer"}"#).expect_err("should refuse");
    assert!(err.to_string().contains("access_token"), "{err}");
}
