use super::*;

const TICK: Duration = Duration::from_millis(60);
/// One frame of the 120 Hz panel this was measured on.
const FRAME: Duration = Duration::from_micros(8333);

#[test]
fn a_fresh_clock_owes_nothing_and_starts_at_the_foot_of_its_first_step() {
    let mut cadence = Cadence::new();
    let start = Instant::now();
    assert_eq!(cadence.owed(start, TICK), 0, "step zero, not step one");
    assert!(cadence.phase(start, TICK).abs() < 1e-3);
    assert_eq!(cadence.due(), Some(start + TICK));
}

#[test]
fn a_display_frame_reads_a_phase_between_two_steps_and_owes_none() {
    let mut cadence = Cadence::new();
    let start = Instant::now();
    cadence.owed(start, TICK);
    // Seven display frames fit inside one tick, and each has to name a
    // different point on the curve — that is the whole claim.
    let mut last = -1.0;
    for frame in 1..=7 {
        let at = start + FRAME * frame;
        assert_eq!(cadence.owed(at, TICK), 0, "still inside the step");
        let phase = cadence.phase(at, TICK);
        assert!(phase > last, "frame {frame}: {phase} <= {last}");
        assert!(phase < 1.0);
        last = phase;
    }
}

#[test]
fn the_step_the_phase_lives_in_is_the_cockpits_own_tick_however_fast_the_display_is() {
    let mut cadence = Cadence::new();
    let start = Instant::now();
    cadence.owed(start, TICK);
    // 120 display frames later is exactly one second: sixteen steps, not
    // one hundred and twenty. A transient's duration is unchanged.
    let mut steps = 0;
    for frame in 1..=120 {
        steps += cadence.owed(start + FRAME * frame, TICK);
    }
    assert_eq!(steps, 16, "one second at 60 ms a step");
}

#[test]
fn a_window_that_was_away_resynchronises_instead_of_replaying_the_absence() {
    let mut cadence = Cadence::new();
    let start = Instant::now();
    cadence.owed(start, TICK);
    let back = start + Duration::from_secs(300);
    assert_eq!(
        cadence.owed(back, TICK),
        Cadence::CATCH_UP,
        "five minutes is not five thousand steps to replay"
    );
    assert_eq!(cadence.due(), Some(back + TICK));
}

#[test]
fn a_slow_frame_is_caught_up_rather_than_dropped() {
    let mut cadence = Cadence::new();
    let start = Instant::now();
    cadence.owed(start, TICK);
    // A frame that took two and a half ticks owes both of the steps it
    // ran through, and the clock stays on the tick's own phase rather
    // than restarting from the late frame.
    let late = start + TICK * 2 + TICK / 2;
    assert_eq!(cadence.owed(late, TICK), 2);
    assert_eq!(cadence.due(), Some(start + TICK * 3));
}

#[test]
fn a_stopped_clock_has_no_phase_so_a_settled_frame_is_the_frame_the_goldens_pin() {
    let mut cadence = Cadence::new();
    let start = Instant::now();
    cadence.owed(start, TICK);
    cadence.stop();
    assert_eq!(cadence.phase(start + FRAME, TICK), 0.0);
    assert_eq!(cadence.due(), None);
    assert_eq!(
        cadence.owed(start + TICK * 9, TICK),
        0,
        "restarted, not owed"
    );
}

#[test]
fn the_loop_is_paced_by_the_display_and_not_by_the_swapchain() {
    // qs-gpu prefers `Mailbox`, which never blocks, so the interval has
    // to be named rather than inherited from a present that returns at
    // once. An unidentified display gets 60 Hz — at least the tick it
    // replaces, and never more frames than can be seen.
    let mut cadence = Cadence::new();
    let start = Instant::now();
    assert_eq!(cadence.frame(start), start + Cadence::DEFAULT_REFRESH);
    cadence.set_refresh(Some(120.0));
    let interval = cadence.frame(start) - start;
    assert!(
        (interval.as_secs_f32() - 1.0 / 120.0).abs() < 1e-5,
        "{interval:?}"
    );
    // A display that reports nothing usable leaves the assumption alone
    // rather than pacing the window at one frame a second.
    cadence.set_refresh(None);
    cadence.set_refresh(Some(0.0));
    assert_eq!(cadence.frame(start) - start, interval);
}
