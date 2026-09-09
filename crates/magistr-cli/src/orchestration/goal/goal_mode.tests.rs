use super::*;

#[test]
fn native_is_the_default_when_the_variable_is_unset_or_empty() {
    assert_eq!(GoalMode::from_value(None), (GoalMode::Native, None));
    assert_eq!(GoalMode::from_value(Some("")), (GoalMode::Native, None));
    assert_eq!(GoalMode::default(), GoalMode::Native);
}

#[test]
fn every_mode_parses_case_insensitively() {
    for (value, mode) in [
        ("native", GoalMode::Native),
        ("Extension", GoalMode::Extension),
        ("COMMAND", GoalMode::Command),
        (" auto ", GoalMode::Auto),
        ("off", GoalMode::Off),
        ("none", GoalMode::Off),
    ] {
        assert_eq!(GoalMode::from_value(Some(value)), (mode, None), "{value}");
    }
}

#[test]
fn an_unknown_value_is_native_and_says_so() {
    let (mode, note) = GoalMode::from_value(Some("delegate"));
    assert_eq!(mode, GoalMode::Native);
    let note = note.expect("an unknown value is reported, never silent");
    assert!(note.contains("MAGISTR_GOAL"), "{note}");
    assert!(note.contains("delegate"), "{note}");
    assert!(note.contains("native"), "{note}");
}

#[test]
fn only_the_three_agent_modes_are_delegated() {
    assert!(!GoalMode::Native.is_delegated());
    assert!(!GoalMode::Off.is_delegated());
    assert!(GoalMode::Extension.is_delegated());
    assert!(GoalMode::Command.is_delegated());
    assert!(GoalMode::Auto.is_delegated());
}

#[test]
fn the_label_round_trips_through_parse() {
    for mode in [
        GoalMode::Native,
        GoalMode::Extension,
        GoalMode::Command,
        GoalMode::Auto,
        GoalMode::Off,
    ] {
        assert_eq!(GoalMode::parse(mode.label()), Some(mode));
    }
}
