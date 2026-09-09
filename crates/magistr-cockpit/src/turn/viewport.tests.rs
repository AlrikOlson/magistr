use super::*;

/// The step every test that does not care about the tick scrolls on.
const NOW: u64 = 0;
/// The step a glide begun on [`NOW`] is over.
const SETTLED: u64 = NOW + Viewport::GLIDE_STEPS;

/// Scroll on step [`NOW`] with motion on — what a cockpit under
/// `MotionMode::Full` does.
fn wheel_up(v: &mut Viewport, total: usize, height: usize) {
    v.wheel_up(total, height, NOW as f32, true);
}

fn wheel_down(v: &mut Viewport, total: usize, height: usize) {
    v.wheel_down(total, height, NOW as f32, true);
}

#[test]
fn a_fresh_viewport_follows_the_tail() {
    let v = Viewport::new();
    assert!(v.is_following());
    assert_eq!(v.top(0, 10), 0);
    assert_eq!(v.top(5, 10), 0);
    assert_eq!(v.top(100, 10), 90);
}

#[test]
fn scrolling_up_pins_and_reaching_the_bottom_follows_again() {
    let mut v = Viewport::new();
    wheel_up(&mut v, 100, 10);
    assert!(!v.is_following());
    assert_eq!(v.top(100, 10), 87);
    wheel_up(&mut v, 100, 10);
    assert_eq!(v.top(100, 10), 84);
    wheel_down(&mut v, 100, 10);
    assert_eq!(v.top(100, 10), 87);
    wheel_down(&mut v, 100, 10);
    assert!(v.is_following(), "back at the bottom");
    assert_eq!(v.top(100, 10), 90);
}

#[test]
fn a_pinned_viewport_stays_put_while_rows_arrive() {
    let mut v = Viewport::new();
    v.page_up(100, 10, NOW as f32, true);
    assert_eq!(v.top(100, 10), 81);
    assert_eq!(v.top(500, 10), 81);
}

#[test]
fn pages_overlap_one_row_and_clamp_at_the_top() {
    let mut v = Viewport::new();
    v.page_up(30, 10, NOW as f32, true);
    assert_eq!(v.top(30, 10), 11);
    v.page_up(30, 10, NOW as f32, true);
    assert_eq!(v.top(30, 10), 2);
    v.page_up(30, 10, NOW as f32, true);
    assert_eq!(v.top(30, 10), 0);
    v.page_down(30, 10, NOW as f32, true);
    assert_eq!(v.top(30, 10), 9);
    v.page_down(30, 10, NOW as f32, true);
    assert_eq!(v.top(30, 10), 18);
    v.page_down(30, 10, NOW as f32, true);
    assert!(v.is_following());
}

#[test]
fn home_and_end() {
    let mut v = Viewport::new();
    v.home(100, 10, NOW as f32, true);
    assert_eq!(v.top(100, 10), 0);
    assert!(!v.is_following());
    v.end(100, 10, NOW as f32, true);
    assert!(v.is_following());
    let mut short = Viewport::new();
    short.home(5, 10, NOW as f32, true);
    assert!(short.is_following(), "nothing to scroll: still following");
}

#[test]
fn a_one_row_stage_still_pages() {
    let mut v = Viewport::new();
    v.page_up(10, 1, NOW as f32, true);
    assert_eq!(v.top(10, 1), 8);
    v.page_up(10, 0, NOW as f32, true);
    assert_eq!(v.top(10, 0), 7);
}

#[test]
fn a_top_anchored_viewport_starts_at_the_first_row_and_does_not_follow() {
    let viewport = Viewport::at_top();
    assert!(!viewport.is_following());
    assert_eq!(viewport.top(100, 10), 0);
    // A list shorter than the window still resolves to the only row.
    assert_eq!(viewport.top(4, 10), 0);
}

/// A cell surface holds each frame for a whole tick, so it reads the
/// curve a step ahead: a three-row notch is on screen the frame it was
/// turned — the frame a terminal's own scroll would show — while the
/// glide's tick still runs its two steps and then stops.
#[test]
fn a_wheel_notch_is_on_screen_the_frame_it_was_turned_on_a_cell_surface() {
    let mut v = Viewport::new();
    wheel_up(&mut v, 100, 10);
    assert_eq!(v.top(100, 10), 87);
    assert_eq!(
        v.top_at(100, 10, NOW as f32),
        87,
        "held for the tick, the notch's frame already shows its rows"
    );
    assert!(v.is_gliding(NOW as f32));
    assert!(v.is_gliding((SETTLED - 1) as f32));
    assert!(!v.is_gliding(SETTLED as f32), "and the tick can stop");
    assert_eq!(v.top_at(100, 10, SETTLED as f32), 87);
}

/// A page has further to go: the held sample shows one intermediate
/// frame on the way, then the window — monotonic, never a strobe.
#[test]
fn a_page_glides_through_one_intermediate_frame_on_a_cell_surface() {
    let mut v = Viewport::new();
    v.page_up(100, 10, NOW as f32, true);
    assert_eq!(v.top(100, 10), 81);
    let seen: Vec<usize> = (NOW..=SETTLED)
        .map(|step| v.top_at(100, 10, step as f32))
        .collect();
    assert_eq!(seen.last(), Some(&81), "lands where `top` already was");
    assert!(
        seen.iter().any(|&row| row > 81 && row < 90),
        "one intermediate frame: {seen:?}"
    );
    assert!(seen.windows(2).all(|w| w[0] >= w[1]), "monotonic: {seen:?}");
}

/// A window paints between rows: the curve's own value at the frame,
/// fractional, from the rows the notch left toward the ones it landed
/// on — and exactly the settled row once the glide is over.
#[test]
fn the_paint_between_rows_is_fractional_on_a_window_and_settles_exactly() {
    let mut v = Viewport::new();
    wheel_up(&mut v, 100, 10);
    assert_eq!(
        v.top_between(100, 10, NOW as f32),
        90.0,
        "starts where it was"
    );
    let midway = v.top_between(100, 10, NOW as f32 + 0.5);
    assert!(midway > 87.0 && midway < 90.0, "between rows: {midway}");
    assert!(midway.fract() != 0.0, "a fraction of a row: {midway}");
    let later = v.top_between(100, 10, NOW as f32 + 1.0);
    assert!(later < midway, "still moving: {later} < {midway}");
    assert_eq!(v.top_between(100, 10, SETTLED as f32), 87.0);
    let fresh = Viewport::new();
    assert!(!fresh.is_gliding(NOW as f32));
    assert_eq!(fresh.top_between(100, 10, NOW as f32), 90.0);
}

#[test]
fn a_settled_viewport_paints_exactly_what_it_did_before_the_glide() {
    let mut v = Viewport::new();
    wheel_up(&mut v, 100, 10);
    assert_eq!(v.top_at(100, 10, SETTLED as f32), v.top(100, 10));
    assert_eq!(v.top_between(100, 10, SETTLED as f32), 87.0);
    // And a viewport nothing ever scrolled has no glide to read.
    let fresh = Viewport::new();
    assert!(!fresh.is_gliding(NOW as f32));
    assert_eq!(fresh.top_at(100, 10, NOW as f32), fresh.top(100, 10));
}

/// Ten notches on one step: the window moves thirty rows, the paint is
/// one glide that starts a notch behind it — never a screen behind — and
/// is over exactly one run later.
#[test]
fn a_burst_of_notches_on_one_step_is_one_glide_at_most_a_notch_behind() {
    let mut v = Viewport::new();
    for _ in 0..10 {
        wheel_up(&mut v, 500, 10);
    }
    assert_eq!(v.top(500, 10), 490 - 30, "ten notches of three rows");
    assert_eq!(
        v.top_between(500, 10, NOW as f32),
        463.0,
        "the paint is a notch behind the window, not a windowful"
    );
    assert!(v.is_gliding(NOW as f32));
    assert!(!v.is_gliding(SETTLED as f32));
    assert_eq!(v.top_at(500, 10, SETTLED as f32), 460);
}

/// A notch that lands mid-glide starts its run from where the paint is,
/// within the notch's own travel: the paint never goes back the way it
/// came, and it lands one run after the last notch.
#[test]
fn a_notch_mid_glide_starts_from_where_the_paint_is() {
    let mut v = Viewport::new();
    v.wheel_up(100, 10, 0.0, true);
    let midway = v.top_between(100, 10, 0.5);
    assert!(midway > 87.0 && midway < 90.0, "mid-glide: {midway}");
    v.wheel_up(100, 10, 0.5, true);
    let from = v.top_between(100, 10, 0.5);
    assert!(
        from <= midway,
        "never back the way it came: {from} vs {midway}"
    );
    assert!(
        from >= 87.0,
        "and at most a notch behind the new window: {from}"
    );
    assert_eq!(
        v.top_between(100, 10, 0.5 + Viewport::GLIDE_STEPS as f32),
        84.0
    );
}

#[test]
fn the_clamp_holds_at_both_ends_mid_glide() {
    let mut v = Viewport::new();
    // Home from the tail of a long turn: the glide is capped at one
    // windowful, so it flies in from just off screen rather than
    // strobing through four hundred rows.
    v.home(500, 10, NOW as f32, true);
    assert_eq!(v.top_between(500, 10, NOW as f32), 10.0);
    for step in NOW..=SETTLED {
        let row = v.top_at(500, 10, step as f32);
        assert!(row <= 10, "never further from home than a window: {row}");
    }
    assert_eq!(v.top_at(500, 10, SETTLED as f32), 0);

    // And at the bottom end nothing paints past the last window.
    v.end(500, 10, SETTLED as f32, true);
    for step in SETTLED..=SETTLED + Viewport::GLIDE_STEPS {
        let row = v.top_at(500, 10, step as f32);
        assert!(row <= 490, "never past the tail: {row}");
        let between = v.top_between(500, 10, step as f32);
        assert!(between <= 490.0, "never past the tail: {between}");
    }
    assert_eq!(
        v.top_at(500, 10, (SETTLED + Viewport::GLIDE_STEPS) as f32),
        490
    );
}

#[test]
fn reduced_motion_scrolls_in_one_frame() {
    let mut v = Viewport::new();
    v.wheel_up(100, 10, NOW as f32, false);
    assert!(
        !v.is_gliding(NOW as f32),
        "no glide to run, so no tick to ask for"
    );
    assert_eq!(v.top_at(100, 10, NOW as f32), 87, "there in one frame");
    assert_eq!(v.top_between(100, 10, NOW as f32), 87.0);
    v.page_up(100, 10, NOW as f32, false);
    assert_eq!(v.top_at(100, 10, NOW as f32), 78);
}

#[test]
fn a_glide_survives_rows_arriving_under_a_following_window() {
    let mut v = Viewport::new();
    // At the tail of a growing transcript, End is a no-op for the top
    // but three more rows land mid-glide. The paint still resolves
    // against the window it has now, and still settles on it.
    v.wheel_up(100, 10, NOW as f32, true);
    assert_eq!(
        v.top_between(103, 10, NOW as f32),
        90.0,
        "the lag rides the new tail"
    );
    assert_eq!(v.top_at(103, 10, SETTLED as f32), 87);
}

/// The window re-pinned on its content after the rows moved: the
/// settled top takes the new row, the glide in flight rides along with
/// it, a row at the last window follows the tail again, and a window
/// that was following has nothing to hold.
#[test]
fn hold_re_pins_the_window_on_its_new_row_and_keeps_the_glide() {
    let mut v = Viewport::new();
    wheel_up(&mut v, 100, 10);
    assert_eq!(v.top(100, 10), 87);
    // A card above the window opened by five rows.
    v.hold(92, 105, 10);
    assert_eq!(v.top(105, 10), 92);
    assert_eq!(
        v.top_between(105, 10, NOW as f32),
        95.0,
        "the paint moved with its rows, the lag untouched"
    );
    assert!(v.is_gliding(NOW as f32));
    assert_eq!(v.top_at(105, 10, SETTLED as f32), 92);
    // The card under the window collapsed until the content it was
    // showing is the last window there is: it follows again.
    v.hold(95, 105, 10);
    assert!(v.is_following());
    let mut following = Viewport::new();
    following.hold(3, 100, 10);
    assert!(following.is_following(), "nothing to hold");
}
