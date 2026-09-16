use super::*;

const MS: fn(u64) -> Duration = Duration::from_millis;

#[test]
fn a_fresh_scheduler_wants_one_frame_and_is_idle() {
    let scheduler = FrameScheduler::new();
    assert!(scheduler.is_dirty());
    assert_eq!(scheduler.tier(Instant::now()), Tier::Idle);
    assert_eq!(scheduler.frames(), 0);
    assert_eq!(FrameScheduler::default(), scheduler);
}

#[test]
fn painting_clears_the_request_and_a_request_sets_it_again() {
    let mut scheduler = FrameScheduler::new();
    scheduler.painted();
    assert!(!scheduler.is_dirty());
    assert_eq!(scheduler.frames(), 1);
    scheduler.request();
    assert!(scheduler.is_dirty());
}

#[test]
fn input_makes_it_interactive_for_half_a_second_then_idle() {
    let mut scheduler = FrameScheduler::new();
    scheduler.painted();
    let t0 = Instant::now();
    scheduler.note(Activity::Input, t0);
    assert!(scheduler.is_dirty());
    assert_eq!(scheduler.tier(t0), Tier::Interactive);
    assert_eq!(scheduler.tier(t0 + MS(499)), Tier::Interactive);
    assert_eq!(scheduler.tier(t0 + MS(500)), Tier::Idle);
}

#[test]
fn a_message_makes_it_streaming_for_a_second_and_input_outranks_it() {
    let mut scheduler = FrameScheduler::new();
    let t0 = Instant::now();
    scheduler.note(Activity::Message, t0);
    assert_eq!(scheduler.tier(t0), Tier::Streaming);
    assert_eq!(scheduler.tier(t0 + MS(999)), Tier::Streaming);
    assert_eq!(scheduler.tier(t0 + MS(1000)), Tier::Idle);
    scheduler.note(Activity::Input, t0 + MS(100));
    assert_eq!(scheduler.tier(t0 + MS(200)), Tier::Interactive);
    assert_eq!(scheduler.tier(t0 + MS(700)), Tier::Streaming);
    assert_eq!(scheduler.tier(t0 + MS(1100)), Tier::Idle);
}

#[test]
fn the_periods_are_the_budgets_and_order_by_urgency() {
    assert_eq!(Tier::Interactive.period(), MS(16));
    assert_eq!(Tier::Streaming.period(), MS(33));
    assert_eq!(Tier::Idle.period(), MS(250));
    assert!(Tier::Interactive < Tier::Streaming && Tier::Streaming < Tier::Idle);
}

#[test]
fn a_still_scheduler_animates_nothing_and_paints_at_the_tier() {
    let mut scheduler = FrameScheduler::new();
    let t0 = Instant::now();
    assert!(!scheduler.is_animating());
    assert!(!scheduler.tick_due(t0), "nothing is owed a step");
    assert_eq!(scheduler.period(t0), Tier::Idle.period());
    scheduler.painted();
    assert!(
        !scheduler.wants_frame(),
        "still and clean: park the interval"
    );
}

#[test]
fn animating_wants_frames_forever_and_still_gives_them_back() {
    let mut scheduler = FrameScheduler::new();
    let t0 = Instant::now();
    scheduler.painted();
    scheduler.animate(MS(120), t0);
    assert!(scheduler.is_animating());
    assert!(
        scheduler.wants_frame(),
        "an animating cockpit keeps the interval alive"
    );
    scheduler.still();
    assert!(!scheduler.is_animating());
    assert!(!scheduler.wants_frame());
    assert!(
        !scheduler.tick_due(t0 + MS(1000)),
        "a stopped tick owes nothing"
    );
}

#[test]
fn the_period_is_the_faster_of_the_tier_and_the_animation() {
    let mut scheduler = FrameScheduler::new();
    let t0 = Instant::now();
    scheduler.animate(MS(120), t0);
    // Idle would paint every 250 ms; the spinner asks for 120.
    assert_eq!(scheduler.period(t0 + MS(2000)), MS(120));
    // Interactive already paints faster than the spinner turns.
    scheduler.note(Activity::Input, t0);
    assert_eq!(scheduler.period(t0), Tier::Interactive.period());
}

#[test]
fn a_step_is_owed_once_per_period_and_the_schedule_does_not_drift() {
    let mut scheduler = FrameScheduler::new();
    let t0 = Instant::now();
    scheduler.animate(MS(100), t0);
    assert!(!scheduler.tick_due(t0 + MS(99)), "not yet");
    assert!(scheduler.tick_due(t0 + MS(100)));
    assert!(!scheduler.tick_due(t0 + MS(100)), "the same instant twice");
    assert!(!scheduler.tick_due(t0 + MS(199)));
    assert!(scheduler.tick_due(t0 + MS(205)));
    // The next step is due a period after the one just taken, not after
    // the instant it was noticed: a late firing does not push the rest out.
    assert!(scheduler.tick_due(t0 + MS(305)));
}

#[test]
fn re_declaring_the_same_period_keeps_the_spinner_in_phase() {
    let mut scheduler = FrameScheduler::new();
    let t0 = Instant::now();
    scheduler.animate(MS(100), t0);
    // Every message while streaming re-declares the animation; if that
    // rescheduled, the spinner would never reach its next frame.
    for i in 0..9 {
        scheduler.animate(MS(100), t0 + MS(10 * i));
    }
    assert!(scheduler.tick_due(t0 + MS(100)));
    // A different period does reschedule from the moment it changed.
    scheduler.animate(MS(50), t0 + MS(100));
    assert!(!scheduler.tick_due(t0 + MS(140)));
    assert!(scheduler.tick_due(t0 + MS(150)));
}

#[test]
fn pixel_phase_samples_between_ticks_without_advancing_the_clock() {
    let mut scheduler = FrameScheduler::new();
    let t0 = Instant::now();
    assert_eq!(scheduler.phase(t0), 0.0);
    scheduler.animate(MS(60), t0);
    assert_eq!(scheduler.phase(t0), 0.0);
    assert!((scheduler.phase(t0 + MS(16)) - 16.0 / 60.0).abs() < 0.0001);
    assert_eq!(scheduler.phase(t0 + MS(30)), 0.5);
    assert!(!scheduler.tick_due(t0 + MS(59)));
    assert_eq!(
        scheduler.phase(t0 + MS(70)),
        1.0,
        "late frames stay bounded"
    );
    assert!(scheduler.tick_due(t0 + MS(70)));
    assert!((scheduler.phase(t0 + MS(70)) - 10.0 / 60.0).abs() < 0.0001);
    scheduler.still();
    assert_eq!(scheduler.phase(t0 + MS(80)), 0.0);
}

#[test]
fn a_zero_period_is_clamped_instead_of_panicking_the_interval() {
    let mut scheduler = FrameScheduler::new();
    let t0 = Instant::now();
    scheduler.animate(Duration::ZERO, t0);
    assert_eq!(scheduler.period(t0 + MS(2000)), FrameScheduler::MIN_PERIOD);
    assert!(scheduler.tick_due(t0 + FrameScheduler::MIN_PERIOD));
}
