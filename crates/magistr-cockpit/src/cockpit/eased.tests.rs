use super::*;

#[test]
fn the_first_target_lands_at_once() {
    let mut fill = Eased::new();
    assert!(!fill.is_seeded());
    assert_eq!(fill.at(0.0), 0.0);
    fill.retarget(0.73, 5.0, true);
    assert!(fill.is_seeded());
    assert_eq!(fill.at(5.0), 0.73, "seeded in place, not from zero");
    assert!(!fill.is_settling(5.0), "a seed is not a glide");
}

#[test]
fn a_later_target_glides_out_fast_and_settles_gently() {
    let mut fill = Eased::new();
    fill.retarget(0.0, 0.0, true);
    fill.retarget(1.0, 10.0, true);
    assert!(fill.is_settling(10.0));
    assert_eq!(fill.at(10.0), 0.0);
    let early = fill.at(11.0);
    let late = fill.at((10 + Eased::STEPS - 1) as f32);
    assert!(early > 0.3, "ease-out moves fast first: {early}");
    assert!(late > 0.95 && late < 1.0, "and settles gently: {late}");
    // Monotonic.
    let mut prev = 0.0;
    for step in 10..=10 + Eased::STEPS {
        let v = fill.at(step as f32);
        assert!(v >= prev, "step {step}: {v} < {prev}");
        prev = v;
    }
    assert_eq!(fill.at((10 + Eased::STEPS) as f32), 1.0);
    assert!(!fill.is_settling((10 + Eased::STEPS) as f32));
    assert_eq!(fill.target(), 1.0);
}

#[test]
fn a_retarget_mid_glide_starts_from_where_it_is() {
    let mut fill = Eased::new();
    fill.retarget(0.0, 0.0, true);
    fill.retarget(1.0, 0.0, true);
    let midway = fill.at(2.0);
    fill.retarget(0.5, 2.0, true);
    assert_eq!(fill.at(2.0), midway, "no snap");
    assert!(fill.is_settling(2.0));
    assert_eq!(fill.at((2 + Eased::STEPS) as f32), 0.5);
}

#[test]
fn the_same_target_again_is_a_no_op_and_reduced_motion_snaps() {
    let mut fill = Eased::new();
    fill.retarget(0.25, 0.0, true);
    fill.retarget(0.75, 1.0, true);
    let snapshot = fill;
    fill.retarget(0.75, 3.0, true);
    assert_eq!(fill, snapshot, "re-asking for the target restarts nothing");

    let mut still = Eased::new();
    still.retarget(0.25, 0.0, false);
    still.retarget(0.75, 1.0, false);
    assert_eq!(still.at(1.0), 0.75, "reduced motion lands at once");
    assert!(!still.is_settling(1.0));
}

#[test]
fn a_glide_from_starts_where_it_is_told_rather_than_where_the_curve_left_it() {
    let mut lag = Eased::new();
    // The curve knows nothing (unseeded, sitting at zero) but the value's
    // frame just moved three rows under it.
    lag.glide_from(-3.0, 0.0, 0.0, true);
    assert_eq!(lag.at(0.0), -3.0, "the given start, not the curve's");
    assert!(lag.is_settling(0.0));
    let midway = lag.at(2.0);
    assert!(midway > -3.0 && midway < 0.0, "easing home: {midway}");
    assert_eq!(lag.at(Eased::STEPS as f32), 0.0);
    assert!(!lag.is_settling(Eased::STEPS as f32));
}

#[test]
fn a_glide_from_lands_at_once_under_reduced_motion_and_a_zero_displacement_never_starts() {
    let mut still = Eased::new();
    still.glide_from(-9.0, 0.0, 4.0, false);
    assert_eq!(still.at(4.0), 0.0, "no tick to catch up on");
    assert!(!still.is_settling(4.0));

    let mut same = Eased::new();
    same.glide_from(2.0, 2.0, 4.0, true);
    assert_eq!(same.at(4.0), 2.0);
    assert!(!same.is_settling(4.0), "nothing to travel");
}

/// A value made with [`Eased::spanning`] runs the same ease-out cubic
/// over its own step count: two steps is over at two, and the curve
/// at its midpoint is the seven-step curve's midpoint — the shape is
/// the shape, only the run is shorter. Zero steps is read as one, so a
/// glide always has a duration to divide by.
#[test]
fn a_glide_spanning_its_own_steps_runs_the_same_curve_in_that_many() {
    let mut short = Eased::spanning(2);
    short.glide_from(0.0, 1.0, 0.0, true);
    let mut long = Eased::new();
    long.glide_from(0.0, 1.0, 0.0, true);
    assert!(short.is_settling(1.0));
    assert!(!short.is_settling(2.0), "over in two");
    assert_eq!(short.at(2.0), 1.0);
    assert!(
        (short.at(1.0) - long.at(3.5)).abs() < 1e-6,
        "the midpoint of each run sits on the same curve: {} vs {}",
        short.at(1.0),
        long.at(3.5)
    );
    assert!(long.is_settling(2.0), "the default run is the seven it was");
    let mut instant = Eased::spanning(0);
    instant.glide_from(0.0, 1.0, 0.0, true);
    assert!(instant.is_settling(0.0));
    assert_eq!(instant.at(1.0), 1.0, "zero steps is read as one");
    // A retarget keeps the span it was made with.
    let mut kept = Eased::spanning(2);
    kept.retarget(0.0, 0.0, true);
    kept.retarget(1.0, 5.0, true);
    assert_eq!(kept.at(7.0), 1.0, "still two steps after a retarget");
}

#[test]
fn a_glide_answers_between_two_ticks_and_never_leaves_the_curve_the_ticks_trace() {
    // the whole of "re-timed for a display rather than a tick".
    // A window samples at `ticks + phase`, so the value at 2.5 has to sit
    // strictly between the two whole steps around it — and the whole
    // steps themselves have to be exactly what an integer clock gave.
    let mut fill = Eased::new();
    fill.retarget(0.0, 0.0, true);
    fill.retarget(1.0, 0.0, true);
    for step in 0..Eased::STEPS {
        let (a, b) = (fill.at(step as f32), fill.at(step as f32 + 1.0));
        let between = fill.at(step as f32 + 0.5);
        assert!(
            between > a && between < b,
            "step {step}: {a} < {between} < {b}"
        );
    }
    // Past the end the phase changes nothing: a settled frame is settled
    // whichever moment inside the tick it is drawn at, which is what
    // keeps every still byte-identical.
    let end = Eased::STEPS as f32;
    assert_eq!(fill.at(end), 1.0);
    assert_eq!(fill.at(end + 0.5), 1.0);
    assert!(!fill.is_settling(end + 0.5));
}
