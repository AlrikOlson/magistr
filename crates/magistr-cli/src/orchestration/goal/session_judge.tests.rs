//! The session judge over the in-process fake ACP agent.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use magistr_acp::AcpTransport;
use magistr_tui::harness::{AcpObserved, FakeAcpAgent};

use super::*;
use crate::error::MagistrError;

/// A connector handing every connection to a fresh copy of `agent`, and
/// what each of them observed.
fn fake_connector(agent: FakeAcpAgent) -> (Connector, Arc<Mutex<Vec<Arc<AcpObserved>>>>) {
    let seen: Arc<Mutex<Vec<Arc<AcpObserved>>>> = Arc::new(Mutex::new(Vec::new()));
    let connector = {
        let seen = Arc::clone(&seen);
        Connector::new(move |_config| {
            let (client_end, observed, _agent_task) = agent.clone().spawn();
            seen.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(observed);
            async move {
                let mut transport =
                    AcpTransport::new_with_args(PathBuf::from("fake-acp-agent"), Vec::new(), None)
                        .map_err(|e| MagistrError::Acp(e.to_string()))?;
                transport
                    .connect_over(client_end)
                    .await
                    .map_err(|e| MagistrError::Acp(e.to_string()))?;
                Ok(transport)
            }
        })
    };
    (connector, seen)
}

fn judge(agent: FakeAcpAgent) -> (SessionJudge, Arc<Mutex<Vec<Arc<AcpObserved>>>>) {
    let (connector, seen) = fake_connector(agent);
    (
        SessionJudge::new(connector, "/tmp/fixture", Backend::Acp),
        seen,
    )
}

fn input() -> JudgeInput {
    JudgeInput {
        chunk: "auth-refresh".to_owned(),
        title: "Refresh tokens rotate before expiry".to_owned(),
        acceptance: vec!["A token within 60s of expiry is refreshed".to_owned()],
        gates: Vec::new(),
        transcript_tail: "done".to_owned(),
        iteration: (1, 3),
    }
}

fn observed(seen: &Arc<Mutex<Vec<Arc<AcpObserved>>>>) -> Vec<Arc<AcpObserved>> {
    seen.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn each_verdict_is_a_new_session_asked_exactly_the_prompt_on_the_small_model() {
    let (judge, seen) = judge(FakeAcpAgent::new().reply(r#"{"ok": true, "reason": "met"}"#));
    let input = input();
    assert_eq!(judge.judge(&input).await, Verdict::met("met"));
    assert_eq!(judge.judge(&input).await, Verdict::met("met"));
    let sessions = observed(&seen);
    assert_eq!(sessions.len(), 2, "one fresh agent per verdict");
    for session in &sessions {
        assert_eq!(
            session.prompts(),
            vec![input.prompt()],
            "exactly the prompt"
        );
        assert_eq!(
            session.config_sets(),
            vec![("model".to_owned(), "fast".to_owned())],
            "the smallest model the agent offers"
        );
        assert_eq!(session.cancels(), 0);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_configured_model_wins_over_the_small_one() {
    let (judge, seen) = judge(FakeAcpAgent::new().reply(r#"{"ok": true, "reason": "met"}"#));
    let judge = judge.model("default");
    assert_eq!(judge.judge(&input()).await, Verdict::met("met"));
    assert_eq!(
        observed(&seen)[0].config_sets(),
        vec![("model".to_owned(), "default".to_owned())]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_three_shapes_are_read_bare_and_fenced() {
    let shapes = [
        (
            r#"{"ok": true, "reason": "all green"}"#,
            Verdict::met("all green"),
        ),
        (
            r#"{"ok": false, "reason": "no tests"}"#,
            Verdict::not_yet("no tests"),
        ),
        (
            r#"{"ok": false, "impossible": true, "reason": "contradiction"}"#,
            Verdict::impossible("contradiction"),
        ),
    ];
    for (json, expected) in shapes {
        let (bare, _) = judge(FakeAcpAgent::new().reply(json));
        assert_eq!(bare.judge(&input()).await, expected, "bare {json}");
        let fenced = format!("Here is my verdict:\n```json\n{json}\n```\n");
        let (fenced, _) = judge(FakeAcpAgent::new().reply(&fenced));
        assert_eq!(fenced.judge(&input()).await, expected, "fenced {json}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_chatty_answer_is_not_yet_carrying_the_text() {
    let (judge, _) = judge(FakeAcpAgent::new().reply("Looks good to me, ship it!"));
    let verdict = judge.judge(&input()).await;
    assert!(!verdict.ok && !verdict.impossible);
    assert!(verdict.reason.contains("ship it!"), "{}", verdict.reason);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_tool_the_judge_asks_for_is_refused() {
    let (judge, seen) = judge(
        FakeAcpAgent::new()
            .reply(r#"{"ok": true, "reason": "ran it"}"#)
            .asking_permission("Bash"),
    );
    assert_eq!(judge.judge(&input()).await, Verdict::met("ran it"));
    assert_eq!(
        observed(&seen)[0].outcomes(),
        vec![Some("reject".to_owned())],
        "no tools: the request is refused, never auto-approved"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_judge_that_never_answers_is_not_yet_after_the_timeout() {
    let (judge, seen) = judge(FakeAcpAgent::new().stalling());
    let judge = judge.timeout(Duration::from_millis(200));
    let started = Instant::now();
    let verdict = judge.judge(&input()).await;
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "the loop is not wedged: {:?}",
        started.elapsed()
    );
    assert!(!verdict.ok && !verdict.impossible);
    assert!(
        verdict.reason.contains("did not answer within"),
        "{}",
        verdict.reason
    );
    assert_eq!(observed(&seen)[0].prompts().len(), 1, "it was asked");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_connection_that_fails_is_not_yet_with_the_error() {
    let connector =
        Connector::new(|_config| async { Err(MagistrError::Acp("no agent on PATH".to_owned())) });
    let judge = SessionJudge::new(connector, "/tmp/fixture", Backend::Acp);
    let verdict = judge.judge(&input()).await;
    assert!(!verdict.ok && !verdict.impossible);
    assert!(
        verdict.reason.contains("no agent on PATH"),
        "{}",
        verdict.reason
    );
}
