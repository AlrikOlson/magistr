use super::*;

#[test]
fn parse_names_both_modes() {
    assert_eq!(MotionMode::parse("full"), Some(MotionMode::Full));
    assert_eq!(MotionMode::parse(" ON "), Some(MotionMode::Full));
    assert_eq!(MotionMode::parse("reduced"), Some(MotionMode::Reduced));
    assert_eq!(MotionMode::parse("off"), Some(MotionMode::Reduced));
    assert_eq!(MotionMode::parse("none"), Some(MotionMode::Reduced));
    assert_eq!(MotionMode::parse("fast"), None);
}

#[test]
fn the_cockpits_own_variable_wins() {
    assert_eq!(
        MotionMode::detect(Some("full"), ["1"]),
        MotionMode::Full,
        "MAGISTR_MOTION=full overrides REDUCE_MOTION=1"
    );
    assert_eq!(MotionMode::detect(Some("reduced"), []), MotionMode::Reduced);
}

#[test]
fn the_reduce_conventions_ask_for_reduced_motion() {
    assert_eq!(MotionMode::detect(None, ["1"]), MotionMode::Reduced);
    assert_eq!(MotionMode::detect(None, ["", "true"]), MotionMode::Reduced);
    // Empty and "0" are the conventions' own "not set".
    assert_eq!(MotionMode::detect(None, ["", "0"]), MotionMode::Full);
}

#[test]
fn nothing_set_is_the_full_layer() {
    assert_eq!(MotionMode::detect(None, []), MotionMode::Full);
    assert_eq!(MotionMode::detect(Some("shrug"), []), MotionMode::Full);
    assert_eq!(MotionMode::default(), MotionMode::Full);
    assert!(MotionMode::Full.is_full());
    assert!(!MotionMode::Reduced.is_full());
}
