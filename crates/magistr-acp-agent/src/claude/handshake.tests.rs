use super::*;
use crate::claude::frame::Frame;

const INIT: &str = include_str!("../../tests/fixtures/claude/init.ndjson");

fn recorded() -> Handshake {
    let line = INIT
        .lines()
        .find(|l| !l.starts_with(r#"{"_sent""#))
        .expect("the response frame");
    let Frame::ControlResponse(response) = Frame::parse(line) else {
        panic!("the fixture is the initialize response");
    };
    assert_eq!(response.request_id, "init-1");
    Handshake::parse(&response.payload)
}

#[test]
fn the_recorded_catalog_lists_the_five_models_fable_included() {
    let handshake = recorded();
    let ids: Vec<&str> = handshake.models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "default",
            "opus[1m]",
            "claude-fable-5-1[1m]",
            "sonnet",
            "haiku"
        ]
    );
    assert_eq!(handshake.models[2].label, "Fable");
    assert_eq!(handshake.models[0].label, "Default (recommended)");
}

#[test]
fn the_recorded_commands_include_goal_and_carry_argument_hints_when_given() {
    let handshake = recorded();
    let goal = handshake
        .commands
        .iter()
        .find(|c| c.name == "goal")
        .expect("the goal command");
    assert!(goal.description.contains("goal"), "{}", goal.description);
    let with_hint = handshake
        .commands
        .iter()
        .find(|c| c.input.is_some())
        .expect("a command with an argument hint");
    let Some(AvailableCommandInput::Unstructured(hint)) = &with_hint.input else {
        panic!("unstructured hint");
    };
    assert!(!hint.hint.is_empty());
}

#[test]
fn an_empty_or_malformed_payload_is_an_empty_handshake() {
    assert_eq!(
        Handshake::parse(&serde_json::json!({})),
        Handshake::default()
    );
    assert_eq!(
        Handshake::parse(&serde_json::json!({"models": "nope", "commands": 3})),
        Handshake::default()
    );
    let bare = Handshake::parse(&serde_json::json!({"models": [{"value": "x"}]}));
    assert_eq!(bare.models, vec![ModelChoice::new("x", "x")]);
}
