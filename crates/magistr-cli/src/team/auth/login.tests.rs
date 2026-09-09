use super::*;

use std::cell::RefCell;

use chrono::{DateTime, Utc};

use crate::team::auth::granted_token::GrantedToken;

/// A scripted provider: `start` answers once, `poll` walks a queue.
struct FakeFlow {
    auth: DeviceAuthorization,
    polls: RefCell<Vec<TokenPoll>>,
    calls: RefCell<Vec<String>>,
    start_error: Option<String>,
}

impl FakeFlow {
    fn new(interval: Option<u64>, expires_in: u64, polls: Vec<TokenPoll>) -> Self {
        FakeFlow {
            auth: DeviceAuthorization {
                device_code: "dc".into(),
                user_code: "WDJB-MJHT".into(),
                verification_uri: "https://example.dev/device".into(),
                verification_uri_complete: None,
                expires_in,
                interval,
            },
            polls: RefCell::new(polls),
            calls: RefCell::new(Vec::new()),
            start_error: None,
        }
    }

    fn failing_start(message: &str) -> Self {
        let mut flow = FakeFlow::new(Some(5), 900, Vec::new());
        flow.start_error = Some(message.to_string());
        flow
    }
}

impl DeviceFlow for FakeFlow {
    fn start(&self) -> Result<DeviceAuthorization, MagistrError> {
        match &self.start_error {
            Some(message) => Err(MagistrError::Other(message.clone())),
            None => Ok(self.auth.clone()),
        }
    }

    fn poll(&self, device_code: &str) -> Result<TokenPoll, MagistrError> {
        self.calls.borrow_mut().push(device_code.to_string());
        let mut polls = self.polls.borrow_mut();
        if polls.is_empty() {
            return Ok(TokenPoll::Pending);
        }
        Ok(polls.remove(0))
    }

    fn identify(&self, token: &GrantedToken) -> Result<VerifiedIdentity, MagistrError> {
        Ok(VerifiedIdentity {
            provider: "fake".into(),
            subject: token.access_token.clone(),
            name: Some("Alice".into()),
            email: Some("alice@example.dev".into()),
            issued_at: epoch(),
            expires_at: epoch(),
        })
    }
}

fn epoch() -> DateTime<Utc> {
    DateTime::from_timestamp(0, 0).expect("timestamp")
}

fn granted(token: &str) -> TokenPoll {
    TokenPoll::Granted(Box::new(GrantedToken {
        access_token: token.to_string(),
        id_token: None,
        expires_in: None,
    }))
}

/// Run `login` against a fake, recording the prompt and every sleep.
fn run(
    flow: &FakeFlow,
) -> (
    Result<VerifiedIdentity, MagistrError>,
    Vec<String>,
    Vec<Duration>,
) {
    let mut prompts = Vec::new();
    let mut sleeps = Vec::new();
    let result = login(
        flow,
        &mut |auth| prompts.push(format!("{} @ {}", auth.user_code, auth.verification_uri)),
        &mut |d| sleeps.push(d),
    );
    (result, prompts, sleeps)
}

#[test]
fn a_grant_on_the_first_poll_returns_the_identity() {
    let flow = FakeFlow::new(Some(5), 900, vec![granted("at")]);
    let (result, prompts, sleeps) = run(&flow);
    let identity = result.expect("login");
    assert_eq!(identity.subject, "at");
    assert_eq!(prompts, vec!["WDJB-MJHT @ https://example.dev/device"]);
    assert_eq!(sleeps, vec![Duration::from_secs(5)]);
}

#[test]
fn the_human_is_prompted_exactly_once_however_long_it_takes() {
    let flow = FakeFlow::new(
        Some(5),
        900,
        vec![TokenPoll::Pending, TokenPoll::Pending, granted("at")],
    );
    let (result, prompts, sleeps) = run(&flow);
    assert!(result.is_ok());
    assert_eq!(prompts.len(), 1);
    assert_eq!(sleeps.len(), 3);
}

#[test]
fn slow_down_widens_every_later_wait() {
    let flow = FakeFlow::new(
        Some(5),
        900,
        vec![TokenPoll::SlowDown, TokenPoll::Pending, granted("at")],
    );
    let (result, _, sleeps) = run(&flow);
    assert!(result.is_ok());
    assert_eq!(
        sleeps,
        vec![
            Duration::from_secs(5),
            Duration::from_secs(10),
            Duration::from_secs(10)
        ]
    );
}

#[test]
fn every_poll_carries_the_device_code() {
    let flow = FakeFlow::new(Some(5), 900, vec![TokenPoll::Pending, granted("at")]);
    let (result, _, _) = run(&flow);
    assert!(result.is_ok());
    assert_eq!(flow.calls.borrow().as_slice(), ["dc", "dc"]);
}

#[test]
fn a_denial_is_a_permission_error_and_stops_polling() {
    let flow = FakeFlow::new(Some(5), 900, vec![TokenPoll::Denied]);
    let (result, _, _) = run(&flow);
    let err = result.expect_err("should refuse");
    assert!(matches!(err, MagistrError::Permission(_)), "{err:?}");
    assert!(err.to_string().contains("denied"), "{err}");
    assert_eq!(flow.calls.borrow().len(), 1);
}

#[test]
fn a_provider_expiry_ends_the_loop() {
    let flow = FakeFlow::new(Some(5), 900, vec![TokenPoll::Expired]);
    let (result, _, _) = run(&flow);
    assert!(
        result
            .expect_err("should refuse")
            .to_string()
            .contains("expired")
    );
}

#[test]
fn a_human_who_never_answers_runs_out_of_device_code_lifetime() {
    // Twelve seconds of lifetime at five-second polls: two waits, then out.
    let flow = FakeFlow::new(Some(5), 12, Vec::new());
    let (result, _, sleeps) = run(&flow);
    let err = result.expect_err("should give up");
    assert!(err.to_string().contains("12s"), "{err}");
    assert!(err.to_string().contains("magistr login"), "{err}");
    assert_eq!(sleeps.len(), 2, "must not poll a code the provider forgot");
}

#[test]
fn a_failure_to_start_never_prompts_the_human() {
    let flow = FakeFlow::failing_start("issuer unreachable");
    let (result, prompts, sleeps) = run(&flow);
    assert!(
        result
            .expect_err("should fail")
            .to_string()
            .contains("unreachable")
    );
    assert!(prompts.is_empty());
    assert!(sleeps.is_empty());
}
