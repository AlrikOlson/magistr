use super::*;

fn json(text: &str) -> Value {
    serde_json::from_str(text).expect("valid json")
}

#[test]
fn discovery_url_appends_the_well_known_path() {
    assert_eq!(
        OidcMetadata::discovery_url("https://id.example.dev"),
        "https://id.example.dev/.well-known/openid-configuration"
    );
}

#[test]
fn discovery_url_does_not_double_the_slash() {
    assert_eq!(
        OidcMetadata::discovery_url("https://id.example.dev/"),
        "https://id.example.dev/.well-known/openid-configuration"
    );
}

#[test]
fn reads_the_two_endpoints_and_ignores_the_rest() {
    let metadata = OidcMetadata::from_json(&json(
        r#"{"issuer":"https://id.example.dev",
            "authorization_endpoint":"https://id.example.dev/auth",
            "device_authorization_endpoint":"https://id.example.dev/device",
            "token_endpoint":"https://id.example.dev/token",
            "jwks_uri":"https://id.example.dev/jwks", "id_token_signing_alg_values_supported":["RS256"]}"#,
    ))
    .expect("parse");
    assert_eq!(
        metadata.device_authorization_endpoint,
        "https://id.example.dev/device"
    );
    assert_eq!(metadata.token_endpoint, "https://id.example.dev/token");
}

#[test]
fn an_issuer_without_the_device_grant_is_refused_by_name() {
    let err = OidcMetadata::from_json(&json(
        r#"{"issuer":"https://id.example.dev","token_endpoint":"https://id.example.dev/token"}"#,
    ))
    .expect_err("should refuse");
    assert!(
        err.to_string().contains("device_authorization_endpoint"),
        "{err}"
    );
}

#[test]
fn a_missing_token_endpoint_is_refused() {
    let err = OidcMetadata::from_json(&json(
        r#"{"device_authorization_endpoint":"https://id.example.dev/device"}"#,
    ))
    .expect_err("should refuse");
    assert!(err.to_string().contains("token_endpoint"), "{err}");
}

#[test]
fn insecure_provider_endpoints_are_refused() {
    let value = serde_json::json!({
        "issuer": "https://id.example.dev",
        "device_authorization_endpoint": "https://id.example.dev/device",
        "token_endpoint": "http://id.example.dev/token",
        "jwks_uri": "https://id.example.dev/keys",
        "id_token_signing_alg_values_supported": ["RS256"]
    });
    assert!(OidcMetadata::from_json(&value).is_err());
}

#[test]
fn missing_jwks_is_refused() {
    let value = serde_json::json!({
        "issuer": "https://id.example.dev",
        "device_authorization_endpoint": "https://id.example.dev/device",
        "token_endpoint": "https://id.example.dev/token",
        "id_token_signing_alg_values_supported": ["RS256"]
    });
    assert!(OidcMetadata::from_json(&value).is_err());
}

#[test]
fn discovery_issuer_and_all_trust_inputs_are_checked() {
    let valid = serde_json::json!({
        "issuer":"https://id.example.dev/",
        "device_authorization_endpoint":"https://device.example.dev/",
        "token_endpoint":"https://token.example.dev/",
        "jwks_uri":"https://keys.example.dev/",
        "id_token_signing_alg_values_supported":["RS256", "ES256"]
    });
    let metadata = OidcMetadata::from_json(&valid).expect("metadata");
    assert!(metadata.validate_issuer("https://id.example.dev/").is_ok());
    assert!(metadata.validate_issuer("https://id.example.dev").is_err());
    for key in [
        "issuer",
        "device_authorization_endpoint",
        "token_endpoint",
        "jwks_uri",
        "id_token_signing_alg_values_supported",
    ] {
        for replacement in [Value::Null, serde_json::json!("http://insecure.test/")] {
            let mut value = valid.clone();
            value[key] = replacement;
            assert!(OidcMetadata::from_json(&value).is_err(), "{key}");
        }
    }
    let mut value = valid;
    value["id_token_signing_alg_values_supported"] = serde_json::json!(["HS256", "none"]);
    assert!(OidcMetadata::from_json(&value).is_err());
}
