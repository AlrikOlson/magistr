//! The one rule that admits a verdict: a confirmed `EndTurn` and a
//! settled cleanup, nothing less.

use agent_client_protocol::schema::v1::StopReason;

use super::*;
use crate::agent::session_supervisor::TurnOutcome;

const MET: &str = r#"{"ok": true, "reason": "met"}"#;

fn outcome(stop_reason: Option<StopReason>, error: Option<&str>) -> Option<TurnOutcome> {
    Some(TurnOutcome {
        turn_id: 1,
        stop_reason,
        error: error.map(str::to_owned),
    })
}

fn evidence(outcome: Option<TurnOutcome>, cleanup: Result<(), String>) -> JudgeEvidence {
    JudgeEvidence { outcome, cleanup }
}

#[test]
fn a_confirmed_end_turn_with_settled_cleanup_admits_the_text() {
    let evidence = evidence(outcome(Some(StopReason::EndTurn), None), Ok(()));
    assert_eq!(evidence.admit(MET), Ok(Verdict::met("met")));
}

#[test]
fn a_cancelled_turn_cannot_admit_ok_even_with_a_full_verdict_in_the_text() {
    let evidence = evidence(outcome(Some(StopReason::Cancelled), None), Ok(()));
    let refused = evidence.admit(MET).expect_err("cancelled is not a verdict");
    assert!(refused.contains("cancelled"), "{refused}");
    assert!(
        refused.contains(MET),
        "the text is kept for the record: {refused}"
    );
}

#[test]
fn a_limit_or_refusal_cannot_admit_ok() {
    for limit in [
        StopReason::MaxTokens,
        StopReason::MaxTurnRequests,
        StopReason::Refusal,
    ] {
        let evidence = evidence(outcome(Some(limit), None), Ok(()));
        let refused = evidence
            .admit(MET)
            .expect_err("a limit or refusal is not a verdict");
        assert!(
            refused.contains(&format!("{limit:?}")),
            "the stop reason is named: {refused}"
        );
    }
}

#[test]
fn a_stream_that_failed_or_ended_cannot_admit_ok() {
    let evidence = evidence(
        outcome(
            None,
            Some("ACP event stream ended without session termination"),
        ),
        Ok(()),
    );
    let refused = evidence
        .admit(MET)
        .expect_err("stream loss is not a verdict");
    assert!(
        refused.contains("ACP event stream ended without session termination"),
        "{refused}"
    );
}

#[test]
fn no_terminal_outcome_cannot_admit_ok() {
    let evidence = evidence(None, Ok(()));
    let refused = evidence
        .admit(MET)
        .expect_err("no outcome is not a verdict");
    assert!(refused.contains("before it answered"), "{refused}");
}

#[test]
fn a_cleanup_failure_cannot_admit_ok_even_after_a_confirmed_end_turn() {
    let evidence = evidence(
        outcome(Some(StopReason::EndTurn), None),
        Err("ACP process cleanup incomplete: group still alive".to_owned()),
    );
    let refused = evidence
        .admit(MET)
        .expect_err("an unsettled session is not evidence");
    assert!(
        refused.contains("ACP process cleanup incomplete"),
        "{refused}"
    );
}

#[test]
fn an_end_turn_that_also_carries_an_error_is_not_confirmed() {
    let evidence = evidence(
        outcome(Some(StopReason::EndTurn), Some("Session owner dropped")),
        Ok(()),
    );
    assert!(evidence.admit(MET).is_err());
}

#[test]
fn the_admitted_text_is_still_read_by_shape() {
    // Evidence admits the *reading* of the text, never a verdict of its
    // own: a chatty answer over perfect evidence is still not yet.
    let evidence = evidence(outcome(Some(StopReason::EndTurn), None), Ok(()));
    let verdict = evidence
        .admit("ship it")
        .expect("valid evidence admits the reading");
    assert!(!verdict.ok && !verdict.impossible);
    assert!(verdict.reason.contains("ship it"));
}
