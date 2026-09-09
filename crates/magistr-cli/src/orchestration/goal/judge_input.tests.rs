use super::*;

use std::time::Duration;

fn input() -> JudgeInput {
    JudgeInput {
        chunk: "auth-refresh".to_owned(),
        title: "Refresh tokens rotate before expiry".to_owned(),
        acceptance: vec![
            "A token within 60s of expiry is refreshed on the next request".to_owned(),
            "A refresh failure surfaces as a re-login prompt, never a silent 401".to_owned(),
        ],
        gates: vec![
            GateEvidence {
                name: "check".to_owned(),
                passed: true,
                duration: Duration::from_secs(3),
            },
            GateEvidence {
                name: "test".to_owned(),
                passed: false,
                duration: Duration::from_secs(40),
            },
        ],
        transcript_tail: "I ran the tests and they all pass.".to_owned(),
        iteration: (2, 5),
    }
}

#[test]
fn the_prompt_carries_every_criterion_verbatim_and_numbered() {
    let prompt = input().prompt();
    assert!(prompt.contains("1. A token within 60s of expiry is refreshed on the next request"));
    assert!(
        prompt.contains("2. A refresh failure surfaces as a re-login prompt, never a silent 401")
    );
    assert!(prompt.contains("`auth-refresh`"));
    assert!(prompt.contains("Refresh tokens rotate before expiry"));
    assert!(prompt.contains("iteration 2 of 5"));
}

#[test]
fn the_prompt_carries_the_gates_as_structured_lines_beside_the_agents_claim() {
    let prompt = input().prompt();
    assert!(prompt.contains("- check: PASSED (3.0s)"));
    assert!(prompt.contains("- test: FAILED (40.0s)"));
    assert!(prompt.contains("real exit codes"));
    // The agent's claim is there too — as words, under its own heading.
    assert!(prompt.contains("I ran the tests and they all pass."));
}

#[test]
fn the_prompt_says_insufficient_evidence_is_not_yet_and_names_the_schema() {
    let prompt = input().prompt();
    assert!(prompt.contains("insufficient evidence"));
    assert!(prompt.contains(Verdict::SCHEMA));
    assert!(prompt.contains("\"impossible\": true"));
}

#[test]
fn an_empty_acceptance_list_and_no_gates_are_said_rather_than_left_blank() {
    let prompt = JudgeInput::default().prompt();
    assert!(prompt.contains("lists none"));
    assert!(prompt.contains("no gates are configured"));
}

#[test]
fn a_long_transcript_is_cut_to_its_tail_at_a_char_boundary() {
    let mut input = input();
    input.transcript_tail = format!(
        "{}é{}",
        "x".repeat(JudgeInput::TAIL_BYTES),
        "end".repeat(10)
    );
    let prompt = input.prompt();
    assert!(prompt.ends_with("must decide.\n"));
    assert!(prompt.contains("endendend"));
    assert!(
        !prompt.contains(&"x".repeat(JudgeInput::TAIL_BYTES)),
        "the head is cut"
    );
    assert!(
        prompt.contains('é'),
        "the cut lands on a char boundary, keeping the é"
    );
}
