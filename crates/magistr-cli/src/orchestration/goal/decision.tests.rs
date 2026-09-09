use super::*;

use std::time::Duration;

fn gate(name: &str, passed: bool) -> GateEvidence {
    GateEvidence {
        name: name.to_owned(),
        passed,
        duration: Duration::from_secs(1),
    }
}

#[test]
fn met_over_green_gates_completes() {
    let gates = [gate("check", true), gate("test", true), gate("lint", true)];
    assert_eq!(
        Decision::reconcile(&Verdict::met("all criteria hold"), &gates),
        Decision::Complete {
            reason: "all criteria hold".to_owned()
        }
    );
}

#[test]
fn a_red_gate_cannot_be_judged_green_whatever_the_agent_claimed() {
    // The agent said the tests pass; the judge believed it; the gate
    // did not. The real exit code wins: this is a disagreement to
    // report, never a completion.
    let gates = [
        gate("check", true),
        gate("test", false),
        gate("lint", false),
    ];
    let decision =
        Decision::reconcile(&Verdict::met("the agent reports all tests passing"), &gates);
    assert_eq!(
        decision,
        Decision::Disagreement {
            reason: "the agent reports all tests passing".to_owned(),
            failed: vec!["test".to_owned(), "lint".to_owned()],
        }
    );
    assert!(!matches!(decision, Decision::Complete { .. }));
    assert!(decision.continues());
}

#[test]
fn not_yet_continues_with_the_reason_even_over_green_gates() {
    let gates = [gate("check", true)];
    let decision = Decision::reconcile(&Verdict::not_yet("criterion 2 has no test"), &gates);
    assert_eq!(
        decision,
        Decision::Continue {
            reason: "criterion 2 has no test".to_owned()
        }
    );
    assert!(decision.continues());
}

#[test]
fn impossible_stops_regardless_of_the_gates() {
    for gates in [vec![], vec![gate("test", true)], vec![gate("test", false)]] {
        let decision = Decision::reconcile(&Verdict::impossible("contradicts the spec"), &gates);
        assert_eq!(
            decision,
            Decision::Stop {
                reason: "contradicts the spec".to_owned()
            }
        );
        assert!(!decision.continues());
    }
}

#[test]
fn met_with_no_gates_configured_completes() {
    assert!(matches!(
        Decision::reconcile(&Verdict::met("ok"), &[]),
        Decision::Complete { .. }
    ));
}

#[test]
fn the_reason_is_readable_off_every_move() {
    let gates = [gate("test", false)];
    for verdict in [
        Verdict::met("a"),
        Verdict::not_yet("a"),
        Verdict::impossible("a"),
    ] {
        assert_eq!(Decision::reconcile(&verdict, &gates).reason(), "a");
    }
}
