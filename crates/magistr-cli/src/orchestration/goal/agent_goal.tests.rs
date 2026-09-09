//! The vendored shape against the three payloads the live probes
//! recorded: an agent bump that moves any of
//! them fails here, not in a session.

use agent_client_protocol::schema::v1::{Meta, SessionUpdate};

use super::*;

const INITIALIZE_META: &str = include_str!("fixtures/initialize_meta.json");
const AVAILABLE_COMMANDS: &str = include_str!("fixtures/available_commands.json");
const GOAL_ECHO: &str = include_str!("fixtures/goal_echo.json");

fn initialize_meta() -> Meta {
    serde_json::from_str(INITIALIZE_META).expect("the recorded initialize _meta parses")
}

fn update(json: &str) -> SessionUpdate {
    serde_json::from_str(json).expect("the recorded update parses as a SessionUpdate")
}

#[test]
fn the_recorded_advertisement_validates_and_names_the_method() {
    let meta = initialize_meta();
    let advertised = AgentGoal::advertisement(Some(&meta)).expect("0.70.0 advertises goal v1");
    assert_eq!(advertised.version, 1);
    assert_eq!(advertised.control_method, "_session/goal");
    assert_eq!(advertised.actions, ["set", "clear"]);
    assert_eq!(advertised.method_for(AgentGoal::SET), Some("_session/goal"));
    assert_eq!(
        advertised.method_for(AgentGoal::CLEAR),
        Some("_session/goal")
    );
    assert_eq!(
        advertised.method_for("pause"),
        None,
        "an action the agent did not list is never sent"
    );
}

#[test]
fn the_capability_read_is_generic_and_serves_the_other_extensions_too() {
    #[derive(serde::Deserialize, PartialEq, Debug)]
    struct Supported {
        supported: bool,
    }
    let meta = initialize_meta();
    assert_eq!(
        AgentGoal::capability::<Supported>(Some(&meta), "steering"),
        Some(Supported { supported: true })
    );
    assert_eq!(
        AgentGoal::capability::<Supported>(Some(&meta), "jetbrains.air"),
        Some(Supported { supported: true })
    );
    assert_eq!(
        AgentGoal::capability::<Supported>(Some(&meta), "nope"),
        None
    );
    assert_eq!(AgentGoal::capability::<Supported>(None, "steering"), None);
}

#[test]
fn an_advertisement_is_refused_when_it_is_structurally_off() {
    for (why, json) in [
        (
            "unknown version",
            r#"{"goal": {"version": 2, "controlMethod": "_session/goal", "actions": ["set"]}}"#,
        ),
        (
            "a method outside the extension namespace",
            r#"{"goal": {"version": 1, "controlMethod": "session/goal", "actions": ["set"]}}"#,
        ),
        ("no method at all", r#"{"goal": {"version": 1}}"#),
        ("the key holds something else", r#"{"goal": true}"#),
        ("no key", r#"{"steering": {"supported": true}}"#),
    ] {
        let meta: Meta = serde_json::from_str(json).expect("meta");
        assert!(
            AgentGoal::advertisement(Some(&meta)).is_none(),
            "{why}: {json}"
        );
    }
    // An advertisement that lists no `set` parses but cannot be used for one.
    let meta: Meta = serde_json::from_str(
        r#"{"goal": {"version": 1, "controlMethod": "_session/goal", "actions": ["clear"]}}"#,
    )
    .expect("meta");
    let advertised = AgentGoal::advertisement(Some(&meta)).expect("valid");
    assert_eq!(advertised.method_for(AgentGoal::SET), None);
}

#[test]
fn the_goal_command_is_found_by_name_in_the_recorded_command_list() {
    let names = AgentGoal::command_names(&update(AVAILABLE_COMMANDS))
        .expect("an available_commands_update");
    assert_eq!(names, ["compact", "goal", "next", "review"]);
    assert_eq!(
        AgentGoal::find_command(names.iter().map(String::as_str)),
        Some("goal".to_owned())
    );
    assert_eq!(AgentGoal::find_command(["compact", "review"]), None);
    assert_eq!(
        AgentGoal::command_names(&update(GOAL_ECHO)),
        None,
        "only the command list carries names"
    );
}

#[test]
fn the_command_turn_is_the_documented_convention() {
    assert_eq!(
        AgentGoal::command_turn("goal", "tests pass"),
        "/goal tests pass"
    );
    assert_eq!(AgentGoal::command_clear_turn("goal"), "/goal clear");
    assert_eq!(
        AgentGoal::set_params("s-1", "tests pass"),
        serde_json::json!({"sessionId": "s-1", "action": "set", "objective": "tests pass"})
    );
    assert_eq!(
        AgentGoal::clear_params("s-1"),
        serde_json::json!({"sessionId": "s-1", "action": "clear"})
    );
}

#[test]
fn the_recorded_echo_confirms_exactly_its_own_objective() {
    let echo = update(GOAL_ECHO);
    assert!(AgentGoal::echo(
        &echo,
        "you have replied with the single word PONG"
    ));
    assert!(
        !AgentGoal::echo(&echo, "you have replied with PONG"),
        "a different objective is not this delivery's echo"
    );
    assert!(
        !AgentGoal::echo(&update(AVAILABLE_COMMANDS), "anything"),
        "only a session_info_update can be the echo"
    );
}

#[test]
fn the_snapshot_relies_on_the_objective_and_nothing_else() {
    // The recorded echo carries status and controlMethod; the wider
    // declared shape carries more. The parsed snapshot has exactly one
    // field, so nothing downstream can read what the wire never sends.
    let wide = update(
        r#"{"sessionUpdate": "session_info_update", "_meta": {"goal": {
            "objective": "x", "status": "paused", "iterations": 3, "lastReason": "not yet",
            "createdAt": "2026-08-29T22:00:00Z", "tokenBudget": 1, "controlMethod": "_session/goal"
        }}}"#,
    );
    let snapshot: GoalSnapshot =
        AgentGoal::update_capability(&wide, AgentGoal::META_KEY).expect("parses");
    assert_eq!(
        snapshot,
        GoalSnapshot {
            objective: "x".to_owned()
        }
    );
    let json = serde_json::to_value(serde_json::json!({"objective": "x"})).expect("value");
    assert_eq!(
        serde_json::from_value::<GoalSnapshot>(json).expect("the bare objective parses"),
        snapshot
    );
    // A cleared goal is reported as null: not a snapshot.
    let cleared = update(r#"{"sessionUpdate": "session_info_update", "_meta": {"goal": null}}"#);
    assert_eq!(
        AgentGoal::update_capability::<GoalSnapshot>(&cleared, AgentGoal::META_KEY),
        None
    );
    // No snapshot field carries progress: the type has one field.
    let fields = std::mem::size_of::<GoalSnapshot>();
    assert_eq!(fields, std::mem::size_of::<String>());
}

#[test]
fn the_objective_is_canonical_under_the_cap_and_names_the_check() {
    let objective = AgentGoal::compile_objective(Some((
        "acp-goal-extension-vendor",
        "Delegating   the loop\n  to the agent's own /goal",
    )));
    assert!(
        objective.chars().count() <= AgentGoal::OBJECTIVE_CAP,
        "{objective}"
    );
    assert!(objective.starts_with("Done means: roadmap chunk `acp-goal-extension-vendor` is complete (Delegating the loop to the agent's own /goal)"), "{objective}");
    assert!(objective.contains("/next"), "{objective}");
    assert!(
        objective.contains("print its verdict line in the transcript"),
        "{objective}"
    );
    assert!(objective.contains("committed"), "{objective}");
    assert!(!objective.contains("  "), "canonical: {objective}");
    assert!(!objective.contains('\n'), "canonical: {objective}");
}

#[test]
fn a_long_title_is_trimmed_so_the_objective_stays_under_the_cap() {
    let long = "x".repeat(2_000);
    let objective = AgentGoal::compile_objective(Some(("chunk", &long)));
    assert!(
        objective.chars().count() <= AgentGoal::OBJECTIVE_CAP,
        "{}",
        objective.len()
    );
    assert!(objective.contains("…)"), "{objective}");
    assert!(objective.ends_with("committed."), "{objective}");
    let none = AgentGoal::compile_objective(None);
    assert!(
        none.starts_with("Done means: the next roadmap item is complete —"),
        "{none}"
    );
    assert!(none.chars().count() <= AgentGoal::OBJECTIVE_CAP);
}
