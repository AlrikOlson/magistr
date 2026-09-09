use super::super::oidc_test_fixture::OidcTestFixture;
use super::*;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use ureq::http::{Request, Response};
use ureq::middleware::MiddlewareNext;
use ureq::{Body, SendBody};

const ISSUER: &str = "https://id.example.test";
const CLIENT: &str = "magistr-cli";
type Reply = (&'static str, &'static str, u16, Value);
type Replies = Arc<Mutex<VecDeque<Reply>>>;

fn scripted(responses: Vec<Reply>) -> (AuthHttp, Replies) {
    let replies = Arc::new(Mutex::new(VecDeque::from(responses)));
    let server = replies.clone();
    let middleware = move |request: Request<SendBody<'_>>, _: MiddlewareNext<'_>| {
        let (method, url, status, value) = server
            .lock()
            .expect("server")
            .pop_front()
            .expect("unexpected request");
        assert_eq!(request.method().as_str(), method);
        assert_eq!(request.uri().to_string(), url);
        if url == super::super::identity_provider::GITHUB_USER_URL {
            assert_eq!(request.headers()["authorization"], "Bearer github-access");
            assert_eq!(request.headers()["user-agent"], "magistr");
        } else {
            assert!(!request.headers().contains_key("authorization"));
        }
        Ok::<_, ureq::Error>(
            Response::builder()
                .status(status)
                .body(Body::builder().data(value.to_string()))
                .expect("response"),
        )
    };
    let agent = ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(0)
        .http_status_as_error(false)
        .middleware(middleware)
        .build()
        .into();
    (AuthHttp::with_agent(agent), replies)
}

fn provider() -> IdentityProvider {
    IdentityProvider::Oidc {
        issuer: ISSUER.into(),
        client_id: CLIENT.into(),
        scope: None,
    }
}

fn metadata() -> Value {
    serde_json::json!({
        "issuer": ISSUER,
        "device_authorization_endpoint": "https://id.example.test/device",
        "token_endpoint": "https://id.example.test/token",
        "jwks_uri": "https://id.example.test/keys",
        "id_token_signing_alg_values_supported": ["RS256"]
    })
}

fn authorization() -> Value {
    serde_json::json!({"device_code":"device-secret", "user_code":"USER",
        "verification_uri":"https://id.example.test/verify", "expires_in":300, "interval":1})
}

fn login_replies(token_response: Value) -> Vec<Reply> {
    vec![
        (
            "GET",
            "https://id.example.test/.well-known/openid-configuration",
            200,
            metadata(),
        ),
        (
            "POST",
            "https://id.example.test/device",
            200,
            authorization(),
        ),
        ("POST", "https://id.example.test/token", 200, token_response),
    ]
}

fn run_flow(http: AuthHttp) -> Result<VerifiedIdentity, MagistrError> {
    let flow = HttpDeviceFlow::connect_with_http(provider(), http)?;
    super::super::login::login(&flow, &mut |_| {}, &mut |_| {})
}

#[test]
fn signed_oidc_login_uses_verified_expiry_and_exact_subject() {
    let fixture = OidcTestFixture::new("key-1");
    let claims = OidcTestFixture::claims(ISSUER, CLIENT, Utc::now());
    let mut replies = login_replies(serde_json::json!({
        "access_token":"opaque", "id_token":fixture.sign(&claims), "expires_in":999999
    }));
    replies.push((
        "GET",
        "https://id.example.test/keys",
        200,
        serde_json::to_value(fixture.jwks()).expect("keys"),
    ));
    let (http, server) = scripted(replies);
    let identity = run_flow(http).expect("signed login");
    assert_eq!(identity.provider, provider().key());
    assert_eq!(identity.subject, claims["sub"].as_str().expect("subject"));
    assert_eq!(
        identity.expires_at.timestamp(),
        claims["exp"].as_i64().expect("expiry")
    );
    assert!(server.lock().expect("server").is_empty());
}

#[test]
fn successful_github_device_flow_uses_only_the_authenticated_user_endpoint() {
    let mut device = authorization();
    device["verification_uri"] = serde_json::json!("https://github.com/login/device");
    let (http, server) = scripted(vec![
        ("POST", GITHUB_DEVICE_CODE_URL, 200, device),
        (
            "POST",
            GITHUB_TOKEN_URL,
            400,
            serde_json::json!({"error":"authorization_pending"}),
        ),
        (
            "POST",
            GITHUB_TOKEN_URL,
            200,
            serde_json::json!({"access_token":"github-access", "expires_in":60}),
        ),
        (
            "GET",
            super::super::identity_provider::GITHUB_USER_URL,
            200,
            serde_json::json!({"id":123, "login":"alice", "name":"Alice"}),
        ),
    ]);
    let github = IdentityProvider::Github {
        client_id: CLIENT.into(),
        scope: None,
    };
    let flow = HttpDeviceFlow::connect_with_http(github, http).expect("github flow");
    let identity =
        super::super::login::login(&flow, &mut |_| {}, &mut |_| {}).expect("github identity");
    assert_eq!(identity.provider, "github");
    assert_eq!(identity.subject, "123");
    assert_eq!(identity.name.as_deref(), Some("Alice"));
    assert_eq!(
        identity.expires_at - identity.issued_at,
        TimeDelta::seconds(60)
    );
    assert!(server.lock().expect("server").is_empty());
}

#[test]
fn missing_malformed_and_cross_provider_tokens_never_fall_back_to_github() {
    for id_token in [
        None,
        Some(Value::Null),
        Some(serde_json::json!(42)),
        Some(serde_json::json!("")),
        Some(serde_json::json!("broken.jwt")),
    ] {
        let mut grant = serde_json::json!({"access_token":"foreign-token"});
        if let Some(value) = id_token {
            grant["id_token"] = value;
        }
        let (http, server) = scripted(login_replies(grant));
        assert!(run_flow(http).is_err());
        assert!(server.lock().expect("server").is_empty());
    }
    let (http, server) = scripted(vec![]);
    let github = IdentityProvider::Github {
        client_id: CLIENT.into(),
        scope: None,
    };
    let flow = HttpDeviceFlow::connect_with_http(github, http).expect("github flow");
    let forged = GrantedToken {
        access_token: "opaque".into(),
        id_token: Some("header.payload.signature".into()),
        expires_in: None,
    };
    assert!(flow.identify(&forged).is_err());
    assert!(server.lock().expect("server").is_empty());
}

#[test]
fn expired_tampered_or_wrong_audience_login_produces_no_identity() {
    let fixture = OidcTestFixture::new("key-1");
    for case in ["expired", "audience", "signature", "issuer", "subject"] {
        let mut claims = OidcTestFixture::claims(ISSUER, CLIENT, Utc::now());
        match case {
            "expired" => claims["exp"] = serde_json::json!(Utc::now().timestamp() - 1),
            "audience" => claims["aud"] = serde_json::json!("different-client"),
            "issuer" => claims["iss"] = serde_json::json!("https://other.test"),
            "subject" => claims["sub"] = serde_json::json!(""),
            _ => {}
        }
        let token = if case == "signature" {
            OidcTestFixture::other("key-1").sign(&claims)
        } else {
            fixture.sign(&claims)
        };
        let mut replies = login_replies(
            serde_json::json!({"access_token":"opaque", "id_token":token, "expires_in":999999}),
        );
        replies.push((
            "GET",
            "https://id.example.test/keys",
            200,
            serde_json::to_value(fixture.jwks()).expect("keys"),
        ));
        let (http, server) = scripted(replies);
        assert!(run_flow(http).is_err(), "{case}");
        assert!(server.lock().expect("server").is_empty());
    }
}

#[test]
fn discovery_and_jwks_provider_errors_are_terminal() {
    for status in [302, 400, 401, 503] {
        let (http, server) = scripted(vec![(
            "GET",
            "https://id.example.test/.well-known/openid-configuration",
            status,
            metadata(),
        )]);
        assert!(HttpDeviceFlow::connect_with_http(provider(), http).is_err());
        assert!(server.lock().expect("server").is_empty());
    }
    let fixture = OidcTestFixture::new("key-1");
    let token = fixture.sign(&OidcTestFixture::claims(ISSUER, CLIENT, Utc::now()));
    for (status, body) in [
        (503, serde_json::json!({"error":"unavailable"})),
        (200, serde_json::json!({"keys":"malformed"})),
    ] {
        let mut replies =
            login_replies(serde_json::json!({"access_token":"opaque", "id_token":token}));
        replies.push(("GET", "https://id.example.test/keys", status, body));
        let (http, server) = scripted(replies);
        assert!(run_flow(http).is_err());
        assert!(server.lock().expect("server").is_empty());
    }
}

#[test]
fn invalid_configuration_is_rejected_before_any_request() {
    for candidate in [
        IdentityProvider::Oidc {
            issuer: "http://insecure.test".into(),
            client_id: CLIENT.into(),
            scope: None,
        },
        IdentityProvider::Oidc {
            issuer: ISSUER.into(),
            client_id: CLIENT.into(),
            scope: Some("profile email".into()),
        },
        IdentityProvider::Oidc {
            issuer: ISSUER.into(),
            client_id: " ".into(),
            scope: None,
        },
    ] {
        let (http, _) = scripted(vec![]);
        assert!(HttpDeviceFlow::connect_with_http(candidate, http).is_err());
    }
}

#[test]
fn discovery_mismatch_and_unsafe_verification_page_fail_before_prompt() {
    let mut mismatch = metadata();
    mismatch["issuer"] = serde_json::json!("https://another.test");
    let (http, _) = scripted(vec![(
        "GET",
        "https://id.example.test/.well-known/openid-configuration",
        200,
        mismatch,
    )]);
    assert!(HttpDeviceFlow::connect_with_http(provider(), http).is_err());
    let mut device = authorization();
    device["verification_uri"] = serde_json::json!("http://unsafe.test");
    let (http, server) = scripted(vec![
        (
            "GET",
            "https://id.example.test/.well-known/openid-configuration",
            200,
            metadata(),
        ),
        ("POST", "https://id.example.test/device", 200, device),
    ]);
    let flow = HttpDeviceFlow::connect_with_http(provider(), http).expect("metadata");
    assert!(
        super::super::login::login(&flow, &mut |_| panic!("must not prompt"), &mut |_| {}).is_err()
    );
    assert!(server.lock().expect("server").is_empty());
}
