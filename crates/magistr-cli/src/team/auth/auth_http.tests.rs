use super::*;
use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use ureq::http::{HeaderMap, HeaderValue, Request, Response};
use ureq::middleware::MiddlewareNext;
use ureq::{Body, SendBody};

fn response(status: u16, body: impl Into<Vec<u8>>) -> Response<Body> {
    Response::builder()
        .status(status)
        .body(Body::builder().data(body))
        .expect("test response")
}

#[test]
fn transport_requires_https_and_disables_redirects() {
    let http = AuthHttp::new();
    assert!(http.agent.config().https_only());
    assert_eq!(http.agent.config().max_redirects(), 0);
    assert_eq!(
        http.agent.config().timeouts().global,
        Some(Duration::from_secs(30))
    );
}

#[test]
fn accepts_success_object_and_returns_response_cache_lifetime() {
    let mut response = response(200, br#"{"keys":[]}"#.as_slice());
    response
        .headers_mut()
        .insert("cache-control", HeaderValue::from_static("max-age=120"));
    response
        .headers_mut()
        .insert("age", HeaderValue::from_static("20"));
    let (value, ttl) = read_json(response, false).expect("valid response");
    assert_eq!(value, serde_json::json!({"keys": []}));
    assert_eq!(ttl, Duration::from_secs(100));
}

#[test]
fn rejects_redirects_and_error_statuses_even_with_plausible_keys() {
    for status in [301, 302, 303, 307, 308, 400, 401, 403, 404, 429, 500, 503] {
        for allow_oauth_error in [false, true] {
            let result = read_json(
                response(status, br#"{"keys":[]}"#.as_slice()),
                allow_oauth_error,
            );
            let error = result.expect_err("non-success must fail").to_string();
            assert!(error.contains(&status.to_string()), "{status}: {error}");
        }
    }
}

#[test]
fn token_poll_accepts_only_explicit_oauth_errors_at_http_400() {
    for error in [
        "authorization_pending",
        "slow_down",
        "access_denied",
        "expired_token",
    ] {
        let body = serde_json::json!({"error": error}).to_string();
        let (value, _) = read_json(response(400, body), true).expect("OAuth error body");
        assert_eq!(value["error"], error);
    }
    for body in [
        r#"{}"#,
        r#"{"error":null}"#,
        r#"{"error":42}"#,
        r#"{"access_token":"secret"}"#,
    ] {
        assert!(read_json(response(400, body), true).is_err());
    }
    assert!(read_json(response(400, r#"{"error":"authorization_pending"}"#), false).is_err());
    assert!(read_json(response(500, r#"{"error":"authorization_pending"}"#), true).is_err());
}

#[test]
fn rejects_malformed_json_and_non_object_json_without_echoing_body() {
    for body in [
        "",
        "{",
        "[]",
        "null",
        "42",
        "\"secret-token\"",
        "{\"secret-token\":}",
    ] {
        let error = read_json(response(200, body), false)
            .expect_err("invalid object")
            .to_string();
        assert!(
            !error.contains("secret-token"),
            "response contents leaked: {error}"
        );
    }
    assert!(read_json(response(200, vec![0xff, 0xfe]), false).is_err());
}

#[test]
fn enforces_one_mib_boundary_on_actual_bytes() {
    // Two braces plus whitespace form a valid object exactly at the limit.
    let at_limit = format!("{{{}}}", " ".repeat(MAX_BODY as usize - 2));
    assert!(read_json(response(200, at_limit.clone()), false).is_ok());
    let error = read_json(response(200, format!("{at_limit} ")), false)
        .expect_err("one byte over the limit")
        .to_string();
    assert!(error.contains("exceeded 1 MiB"), "{error}");
}

#[test]
fn streamed_body_is_bounded_without_content_length() {
    let oversized = std::io::repeat(b' ').take(MAX_BODY + 1);
    let response = Response::builder()
        .body(Body::builder().reader(oversized))
        .expect("test response");
    assert!(response.body().content_length().is_none());
    let error = read_json(response, false)
        .expect_err("stream limit")
        .to_string();
    assert!(error.contains("exceeded 1 MiB"), "{error}");
}

#[test]
fn unreadable_body_fails_without_transport_detail() {
    struct BrokenReader;
    impl std::io::Read for BrokenReader {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("secret-token transport detail"))
        }
    }
    let response = Response::new(Body::builder().reader(BrokenReader));
    let error = read_json(response, false)
        .expect_err("reader failure")
        .to_string();
    assert!(error.contains("unreadable"));
    assert!(!error.contains("secret-token"));
}

fn cached_for(cache_control: &[&str], age: Option<&str>) -> Duration {
    let mut headers = HeaderMap::new();
    for value in cache_control {
        headers.append(
            "cache-control",
            HeaderValue::from_str(value).expect("test header"),
        );
    }
    if let Some(age) = age {
        headers.insert("age", HeaderValue::from_str(age).expect("test Age"));
    }
    cache_age(&headers)
}

#[test]
fn cache_lifetime_has_five_minute_cap_and_honors_smaller_max_age() {
    for (directives, seconds) in [
        (vec![], 300),
        (vec!["public, max-age=86400"], 300),
        (vec!["max-age=0"], 0),
        (vec!["max-age=10"], 10),
        (vec!["MaX-aGe = \"30\""], 30),
        (vec!["max-age=80", "max-age=20"], 20),
        (vec!["max-age=30, max-age=10"], 10),
    ] {
        assert_eq!(cached_for(&directives, None), Duration::from_secs(seconds));
    }
}

#[test]
fn cache_lifetime_honors_revalidation_and_invalid_max_age() {
    for directives in [
        "no-store",
        "public, NO-CACHE",
        "max-age=60, no-store",
        "max-age=",
        "max-age=-1",
        "max-age=invalid",
        "max-age=18446744073709551616",
    ] {
        assert_eq!(
            cached_for(&[directives], None),
            Duration::ZERO,
            "{directives}"
        );
    }
    assert_eq!(
        cached_for(&["max-age=300", "no-store"], None),
        Duration::ZERO
    );
    let mut headers = HeaderMap::new();
    headers.insert(
        "cache-control",
        HeaderValue::from_bytes(&[0xff]).expect("opaque header"),
    );
    assert_eq!(cache_age(&headers), Duration::ZERO);
}

#[test]
fn age_reduces_remaining_lifetime_and_never_underflows() {
    for (age, seconds) in [
        ("0", 60),
        ("20", 40),
        ("60", 0),
        ("999", 0),
        ("-1", 0),
        ("broken", 0),
    ] {
        assert_eq!(
            cached_for(&["max-age=60"], Some(age)),
            Duration::from_secs(seconds),
            "{age}"
        );
    }
}

#[test]
fn malformed_age_header_does_not_extend_cache_lifetime() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "age",
        HeaderValue::from_bytes(&[0xff]).expect("opaque header"),
    );
    assert_eq!(cache_age(&headers), Duration::ZERO);
}

#[test]
fn rejects_untrusted_endpoint_before_request_is_sent() {
    let calls = Arc::new(AtomicUsize::new(0));
    let request_calls = Arc::clone(&calls);
    let agent = ureq::Agent::config_builder()
        .middleware(move |_: Request<SendBody<'_>>, _: MiddlewareNext<'_>| {
            request_calls.fetch_add(1, Ordering::SeqCst);
            Ok(response(200, "{}"))
        })
        .build()
        .new_agent();
    let http = AuthHttp::with_agent(agent);
    for endpoint in [
        "http://issuer.example/keys",
        "https://user:password@issuer.example/keys",
        "https://issuer.example/keys#fragment",
        "/relative",
    ] {
        assert!(http.get_json(endpoint).is_err(), "{endpoint}");
        assert!(
            http.post_form(endpoint, &[("device_code", "secret")])
                .is_err(),
            "{endpoint}"
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn post_form_preserves_http_400_oauth_poll_response() {
    let agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .middleware(|request: Request<SendBody<'_>>, _: MiddlewareNext<'_>| {
            assert_eq!(request.method(), "POST");
            assert_eq!(request.uri(), "https://issuer.example/token");
            assert_eq!(request.headers()["accept"], "application/json");
            Ok(response(400, r#"{"error":"authorization_pending"}"#))
        })
        .build()
        .new_agent();
    let http = AuthHttp::with_agent(agent);
    let value = http
        .post_form("https://issuer.example/token", &[("device_code", "opaque")])
        .expect("OAuth polling status");
    assert_eq!(value["error"], "authorization_pending");
}
