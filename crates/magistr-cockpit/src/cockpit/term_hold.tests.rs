use super::*;

fn target() -> KillCmd {
    KillCmd::Terminal("term-1".to_owned())
}

/// Half a second on a sixty-millisecond tick is a little over eight
/// steps; the dead zone is two.
#[test]
fn the_hold_is_half_a_second_of_ticks_and_the_dead_zone_a_click() {
    assert!(
        (TermHold::HOLD - 500.0 / 60.0).abs() < 1e-5,
        "{}",
        TermHold::HOLD
    );
    assert!((TermHold::DEAD - 2.0).abs() < 1e-5, "{}", TermHold::DEAD);
}

/// The fill is nothing through the dead zone, climbs from there, and is
/// whole exactly when the hold is done — never before.
#[test]
fn the_fill_waits_out_the_dead_zone_then_climbs_to_the_hold() {
    let hold = TermHold::key(target(), 10.0);
    assert_eq!(hold.fraction(10.0), 0.0);
    assert_eq!(
        hold.fraction(10.0 + TermHold::DEAD),
        0.0,
        "the last dead step"
    );
    assert_eq!(
        hold.fraction(9.0),
        0.0,
        "a step before the press is nothing"
    );
    let mid = hold.fraction(10.0 + (TermHold::DEAD + TermHold::HOLD) / 2.0);
    assert!((mid - 0.5).abs() < 1e-5, "{mid}");
    assert!(!hold.is_done(10.0 + TermHold::HOLD - 0.01));
    assert!(hold.fraction(10.0 + TermHold::HOLD - 0.01) < 1.0);
    assert!(hold.is_done(10.0 + TermHold::HOLD));
    assert_eq!(hold.fraction(10.0 + TermHold::HOLD), 1.0);
    assert_eq!(
        hold.fraction(10.0 + TermHold::HOLD * 3.0),
        1.0,
        "clamped past"
    );
}

/// The two ways in are told apart, and both name what they would kill.
#[test]
fn a_key_hold_and_a_cap_hold_name_their_target_and_their_way_in() {
    let key = TermHold::key(target(), 0.0);
    let cap = TermHold::cap(KillCmd::Gate("test".to_owned()), 0.0);
    assert!(!key.is_by_cap());
    assert!(cap.is_by_cap());
    assert_eq!(key.target(), &target());
    assert_eq!(key.target().id(), "term-1");
    assert_eq!(cap.target().id(), "test");
    assert_ne!(key, cap);
}
