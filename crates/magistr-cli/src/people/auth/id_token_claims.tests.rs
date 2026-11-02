use super::*;

use base64::prelude::*;
use jsonwebtoken::{EncodingKey, Header};
use serde_json::{Value, json};

use super::super::oidc_test_fixture::OidcTestFixture;

const ISSUER: &str = "https://id.example.dev/tenant";
const CLIENT: &str = "magistr";

fn now() -> DateTime<Utc> {
    DateTime::from_timestamp(1_900_000_000, 0).expect("fixture timestamp")
}

fn claims() -> Value {
    OidcTestFixture::claims(ISSUER, CLIENT, now())
}

fn verify(token: &str, jwks: &JwkSet) -> Result<IdentityClaims, MagistrError> {
    id_token_claims(token, jwks, ISSUER, CLIENT, now(), now())
}

#[test]
fn authenticates_signed_standard_identity_claims() {
    let fixture = OidcTestFixture::new("key-1");
    let identity = verify(&fixture.sign(&claims()), &fixture.jwks()).expect("valid token");
    assert_eq!(identity.sub, "user-1");
    assert_eq!(identity.email.as_deref(), Some("alice@example.dev"));
    assert_eq!(identity.display_name(), Some("Alice Ng"));
    assert_eq!(identity.exp, now().timestamp() + 3600);
    assert_eq!(identity.iat, now().timestamp());
}

#[test]
fn display_name_falls_back_and_unknown_claims_are_ignored() {
    let fixture = OidcTestFixture::new("key-1");
    let mut claims = claims();
    claims["name"] = json!("   ");
    claims["preferred_username"] = json!("alice");
    claims["custom_claim"] = json!({"ignored": true});
    let identity = verify(&fixture.sign(&claims), &fixture.jwks()).expect("valid token");
    assert_eq!(identity.display_name(), Some("alice"));
}

#[test]
fn unsigned_provider_token_must_not_authenticate() {
    let fixture = OidcTestFixture::new("key-1");
    let token = format!(
        "{}.{}.not-a-real-signature",
        BASE64_URL_SAFE_NO_PAD.encode(r#"{"alg":"RS256","kid":"key-1"}"#),
        BASE64_URL_SAFE_NO_PAD.encode(claims().to_string()),
    );
    assert!(verify(&token, &fixture.jwks()).is_err());
}

#[test]
fn tampered_payload_and_wrong_signing_key_are_rejected() {
    let fixture = OidcTestFixture::new("key-1");
    let token = fixture.sign(&claims());
    let mut parts: Vec<_> = token.split('.').map(str::to_owned).collect();
    let mut forged = claims();
    forged["sub"] = json!("administrator");
    parts[1] = BASE64_URL_SAFE_NO_PAD.encode(forged.to_string());
    assert!(verify(&parts.join("."), &fixture.jwks()).is_err());
    let attacker = OidcTestFixture::other("key-1");
    assert!(verify(&attacker.sign(&claims()), &fixture.jwks()).is_err());
}

#[test]
fn none_hmac_and_unnegotiated_rsa_algorithms_are_rejected() {
    let fixture = OidcTestFixture::new("key-1");
    let none = format!(
        "{}.{}.",
        BASE64_URL_SAFE_NO_PAD.encode(r#"{"alg":"none","kid":"key-1"}"#),
        BASE64_URL_SAFE_NO_PAD.encode(claims().to_string()),
    );
    assert!(verify(&none, &fixture.jwks()).is_err());
    let mut header = Header::new(Algorithm::HS256);
    header.kid = Some("key-1".into());
    let public_key = serde_json::to_vec(&fixture.jwks()).expect("public key bytes");
    let hmac = jsonwebtoken::encode(&header, &claims(), &EncodingKey::from_secret(&public_key))
        .expect("HMAC confusion fixture");
    assert!(verify(&hmac, &fixture.jwks()).is_err());
    header.alg = Algorithm::RS512;
    assert!(verify(&fixture.sign_header(&header, &claims()), &fixture.jwks()).is_err());
}

#[test]
fn unknown_missing_blank_and_ambiguous_key_ids_are_rejected() {
    let fixture = OidcTestFixture::new("key-1");
    for kid in [None, Some(""), Some("   "), Some("unknown")] {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = kid.map(str::to_owned);
        assert!(verify(&fixture.sign_header(&header, &claims()), &fixture.jwks()).is_err());
    }
    let mut ambiguous = fixture.jwks();
    ambiguous.keys.extend(fixture.jwks().keys);
    assert!(verify(&fixture.sign(&claims()), &ambiguous).is_err());
}

#[test]
fn unsuitable_signing_key_metadata_is_rejected() {
    let fixture = OidcTestFixture::new("key-1");
    let token = fixture.sign(&claims());
    for (field, value) in [
        ("alg", json!("RS512")),
        ("alg", json!("HS256")),
        ("use", json!("enc")),
        ("key_ops", json!([])),
        ("key_ops", json!(["sign"])),
        ("key_ops", json!(["verify", "encrypt"])),
        ("key_ops", json!(["verify", "verify"])),
    ] {
        let mut keys = serde_json::to_value(fixture.jwks()).expect("fixture JSON");
        keys["keys"][0][field] = value;
        let jwks = serde_json::from_value(keys).expect("JWK schema");
        assert!(verify(&token, &jwks).is_err(), "{field}");
    }
    let jwks: JwkSet = serde_json::from_value(json!({"keys": [{
        "kid": "key-1", "kty": "oct", "k": "c2VjcmV0", "alg": "RS256"
    }]}))
    .expect("symmetric JWK");
    assert!(verify(&token, &jwks).is_err());
}

#[test]
fn optional_key_metadata_can_be_omitted() {
    let fixture = OidcTestFixture::new("key-1");
    let mut jwks = fixture.jwks();
    jwks.keys[0].common.key_algorithm = None;
    jwks.keys[0].common.key_operations = None;
    jwks.keys[0].common.public_key_use = None;
    assert!(verify(&fixture.sign(&claims()), &jwks).is_ok());
}

#[test]
fn malformed_rsa_components_fail_closed() {
    let fixture = OidcTestFixture::new("key-1");
    for invalid_component in ["", "!!!", "AQAB"] {
        let mut jwks = fixture.jwks();
        let AlgorithmParameters::RSA(rsa) = &mut jwks.keys[0].algorithm else {
            panic!("RSA fixture");
        };
        rsa.n = invalid_component.into();
        assert!(verify(&fixture.sign(&claims()), &jwks).is_err());
    }
}

#[test]
fn exact_issuer_and_only_the_configured_audience_are_trusted() {
    let fixture = OidcTestFixture::new("key-1");
    for (field, value) in [
        ("iss", json!("https://id.example.dev/tenant/")),
        ("iss", json!("https://ID.example.dev/tenant")),
        ("aud", json!("other-client")),
        ("aud", json!([CLIENT, "other-client"])),
        ("aud", json!([])),
        ("aud", json!([CLIENT, 42])),
        ("azp", json!("other-client")),
    ] {
        let mut claims = claims();
        claims[field] = value;
        assert!(
            verify(&fixture.sign(&claims), &fixture.jwks()).is_err(),
            "{field}"
        );
    }
    for aud in [json!(CLIENT), json!([CLIENT]), json!([CLIENT, CLIENT])] {
        let mut claims = claims();
        claims["aud"] = aud;
        assert!(verify(&fixture.sign(&claims), &fixture.jwks()).is_ok());
        claims["azp"] = json!(CLIENT);
        assert!(verify(&fixture.sign(&claims), &fixture.jwks()).is_ok());
    }
}

#[test]
fn required_claims_cannot_be_missing_null_or_wrong_type() {
    let fixture = OidcTestFixture::new("key-1");
    for field in ["iss", "sub", "aud", "exp", "iat"] {
        let mut missing = claims();
        missing
            .as_object_mut()
            .expect("claims object")
            .remove(field);
        assert!(
            verify(&fixture.sign(&missing), &fixture.jwks()).is_err(),
            "missing {field}"
        );
        for invalid in [Value::Null, json!(true), json!({})] {
            missing[field] = invalid;
            assert!(
                verify(&fixture.sign(&missing), &fixture.jwks()).is_err(),
                "invalid {field}"
            );
        }
    }
    for field in ["exp", "iat"] {
        for invalid in [json!("1900003600"), json!(1_900_003_600.5)] {
            let mut claims = claims();
            claims[field] = invalid;
            assert!(
                verify(&fixture.sign(&claims), &fixture.jwks()).is_err(),
                "numeric {field}"
            );
        }
    }
}

#[test]
fn stable_subject_must_be_nonblank_ascii_and_bounded() {
    let fixture = OidcTestFixture::new("key-1");
    for subject in [
        "".to_owned(),
        "   ".to_owned(),
        "élise".to_owned(),
        "a".repeat(256),
    ] {
        let mut claims = claims();
        claims["sub"] = json!(subject);
        assert!(verify(&fixture.sign(&claims), &fixture.jwks()).is_err());
    }
    let mut claims = claims();
    claims["sub"] = json!("a".repeat(255));
    assert!(verify(&fixture.sign(&claims), &fixture.jwks()).is_ok());
}

#[test]
fn expiry_is_strict_without_leeway() {
    let fixture = OidcTestFixture::new("key-1");
    for offset in [-60, -1, 0, 1] {
        let mut claims = claims();
        claims["exp"] = json!(now().timestamp() + offset);
        assert_eq!(
            verify(&fixture.sign(&claims), &fixture.jwks()).is_ok(),
            offset > 0
        );
    }
}

#[test]
fn issue_time_is_bound_to_the_authorization_attempt_with_limited_skew() {
    let fixture = OidcTestFixture::new("key-1");
    for offset in [-61, -60, 0, 60, 61] {
        let mut claims = claims();
        claims["iat"] = json!(now().timestamp() + offset);
        assert_eq!(
            verify(&fixture.sign(&claims), &fixture.jwks()).is_ok(),
            (-60..=60).contains(&offset)
        );
    }
    let mut claims = claims();
    claims["iat"] = json!(now().timestamp() + 1);
    claims["exp"] = claims["iat"].clone();
    assert!(verify(&fixture.sign(&claims), &fixture.jwks()).is_err());
    claims["iat"] = json!(now().timestamp() - 120);
    claims["exp"] = json!(now().timestamp() + 3600);
    assert!(
        id_token_claims(
            &fixture.sign(&claims),
            &fixture.jwks(),
            ISSUER,
            CLIENT,
            now(),
            now() - chrono::Duration::minutes(5)
        )
        .is_ok()
    );
}

#[test]
fn optional_validity_claims_have_strict_types_and_boundaries() {
    let fixture = OidcTestFixture::new("key-1");
    for field in ["nbf", "azp", "nonce"] {
        for invalid in [Value::Null, json!(true), json!({})] {
            let mut claims = claims();
            claims[field] = invalid;
            assert!(verify(&fixture.sign(&claims), &fixture.jwks()).is_err());
        }
    }
    for offset in [-1, 0, 1] {
        let mut claims = claims();
        claims["nbf"] = json!(now().timestamp() + offset);
        assert_eq!(
            verify(&fixture.sign(&claims), &fixture.jwks()).is_ok(),
            offset <= 0
        );
    }
    let mut claims = claims();
    claims["nonce"] = json!("unexpected");
    assert!(verify(&fixture.sign(&claims), &fixture.jwks()).is_err());
}

#[test]
fn critical_and_payload_encoding_extensions_are_rejected() {
    let fixture = OidcTestFixture::new("key-1");
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("key-1".into());
    header.crit = Some(vec!["unknown".into()]);
    assert!(verify(&fixture.sign_header(&header, &claims()), &fixture.jwks()).is_err());
    header.crit = Some(vec![]);
    assert!(verify(&fixture.sign_header(&header, &claims()), &fixture.jwks()).is_err());
    header.crit = None;
    header.extras.insert("b64", false);
    assert!(verify(&fixture.sign_header(&header, &claims()), &fixture.jwks()).is_err());
}

#[test]
fn header_keys_and_urls_never_replace_discovered_trust() {
    let fixture = OidcTestFixture::new("key-1");
    let attacker = OidcTestFixture::other("key-1");
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("key-1".into());
    header.jku = Some("http://127.0.0.1/attacker-jwks".into());
    header.x5u = Some("http://127.0.0.1/attacker-cert".into());
    header.jwk = Some(attacker.jwks().keys.remove(0));
    assert!(verify(&fixture.sign_header(&header, &claims()), &fixture.jwks()).is_ok());
    assert!(verify(&attacker.sign_header(&header, &claims()), &fixture.jwks()).is_err());
}

#[test]
fn malformed_padded_and_oversized_tokens_fail_closed() {
    let fixture = OidcTestFixture::new("key-1");
    for token in ["", "a.b", "a.b.c.d", "h.!!!.s", "h.W10.s"] {
        assert!(verify(token, &fixture.jwks()).is_err());
    }
    let valid = fixture.sign(&claims());
    let padded = valid.replacen('.', "=.", 1);
    assert!(verify(&padded, &fixture.jwks()).is_err());
    assert!(verify(&"a".repeat(MAX_ID_TOKEN_BYTES + 1), &fixture.jwks()).is_err());
}
