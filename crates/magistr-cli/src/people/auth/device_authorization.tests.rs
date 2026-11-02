use super::*;

fn json(text: &str) -> Value {
    serde_json::from_str(text).expect("valid json")
}

#[test]
fn reads_a_spec_shaped_response() {
    let auth = DeviceAuthorization::from_json(&json(
        r#"{"device_code":"dc","user_code":"WDJB-MJHT",
            "verification_uri":"https://example.dev/device",
            "verification_uri_complete":"https://example.dev/device?user_code=WDJB-MJHT",
            "expires_in":1800,"interval":5}"#,
    ))
    .expect("parse");
    assert_eq!(auth.device_code, "dc");
    assert_eq!(auth.user_code, "WDJB-MJHT");
    assert_eq!(auth.verification_uri, "https://example.dev/device");
    assert_eq!(
        auth.verification_uri_complete.as_deref(),
        Some("https://example.dev/device?user_code=WDJB-MJHT")
    );
    assert_eq!(auth.expires_in, 1800);
    assert_eq!(auth.interval, Some(5));
}

#[test]
fn accepts_the_draft_verification_url_spelling() {
    let auth = DeviceAuthorization::from_json(&json(
        r#"{"device_code":"dc","user_code":"ABCD",
            "verification_url":"https://google.example/device"}"#,
    ))
    .expect("parse");
    assert_eq!(auth.verification_uri, "https://google.example/device");
}

#[test]
fn expires_in_defaults_to_fifteen_minutes() {
    let auth = DeviceAuthorization::from_json(&json(
        r#"{"device_code":"dc","user_code":"ABCD","verification_uri":"https://x.dev"}"#,
    ))
    .expect("parse");
    assert_eq!(auth.expires_in, 900);
    assert_eq!(auth.interval, None);
}

#[test]
fn a_missing_required_field_names_itself() {
    let err = DeviceAuthorization::from_json(&json(
        r#"{"user_code":"ABCD","verification_uri":"https://x.dev"}"#,
    ))
    .expect_err("should refuse");
    assert!(err.to_string().contains("device_code"), "{err}");
}

#[test]
fn a_missing_verification_uri_names_the_spec_spelling() {
    let err = DeviceAuthorization::from_json(&json(r#"{"device_code":"dc","user_code":"A"}"#))
        .expect_err("should refuse");
    assert!(err.to_string().contains("verification_uri"), "{err}");
}
