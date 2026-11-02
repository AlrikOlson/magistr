use super::*;

fn now() -> DateTime<Utc> {
    DateTime::from_timestamp(1_000_000, 0).expect("timestamp")
}

#[test]
fn an_id_token_exp_in_the_future_bounds_the_session() {
    let exp = now().timestamp() + 300;
    assert_eq!(
        expiry(now(), Some(exp), Some(99_999)),
        now() + TimeDelta::seconds(300)
    );
}

#[test]
fn an_id_token_exp_in_the_past_is_never_renewed() {
    let exp = now().timestamp() - 300;
    assert_eq!(
        expiry(now(), Some(exp), Some(60)),
        now() - TimeDelta::seconds(300)
    );
}

#[test]
fn expires_in_is_used_when_there_is_no_id_token() {
    assert_eq!(
        expiry(now(), None, Some(3600)),
        now() + TimeDelta::seconds(3600)
    );
}

#[test]
fn a_provider_that_states_no_lifetime_gets_the_default_session() {
    assert_eq!(
        expiry(now(), None, None),
        now() + TimeDelta::seconds(DEFAULT_SESSION_SECS)
    );
}

#[test]
fn a_zero_expires_in_is_not_extended() {
    assert_eq!(expiry(now(), None, Some(0)), now());
    assert_eq!(
        expiry(now(), None, Some(u64::MAX)),
        now() + TimeDelta::seconds(DEFAULT_SESSION_SECS)
    );
}

#[test]
fn github_requires_a_numeric_immutable_subject() {
    for value in [
        serde_json::json!({"login": "mutable"}),
        serde_json::json!({"id": 0}),
        serde_json::json!({"id": -1}),
        serde_json::json!({"id": "123"}),
    ] {
        assert!(github_identity(&value).is_err());
    }
    assert_eq!(
        github_identity(&serde_json::json!({"id": 123, "login": "alice"}))
            .expect("user")
            .0,
        "123"
    );
}
