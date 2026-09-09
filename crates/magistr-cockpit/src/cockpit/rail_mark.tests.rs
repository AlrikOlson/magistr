use super::*;

/// The first tick at or past `secs` whole seconds after tick 0.
fn tick(secs: u64) -> u64 {
    let period = u64::try_from(Model::TICK_PERIOD.as_millis()).expect("fits");
    (secs * 1_000).div_ceil(period)
}

#[test]
fn the_first_row_ticks_and_stamps_the_origin() {
    let mark = RailMark::of(100, None, 100);
    assert!(mark.tick);
    assert!(!mark.silence);
    assert_eq!(mark.stamp, Some(0));
}

#[test]
fn a_row_in_the_same_second_is_quiet() {
    let mark = RailMark::of(0, Some(3), 9);
    assert!(mark.is_quiet(), "{mark:?}");
}

#[test]
fn a_new_second_ticks_without_a_stamp() {
    let mark = RailMark::of(0, Some(tick(2)), tick(3));
    assert!(mark.tick);
    assert!(!mark.silence);
    assert_eq!(mark.stamp, None, "a plain second is a tick, never a number");
}

#[test]
fn a_silence_over_a_second_breaks_and_stamps() {
    let mark = RailMark::of(0, Some(tick(1)), tick(1) + tick(1) + 2);
    assert!(mark.silence);
    assert!(mark.tick, "a gap over a second always crosses one");
    assert_eq!(mark.stamp, Some(2));
    // A gap under a second — sixteen ticks, 960 ms — is not a silence.
    let close = RailMark::of(0, Some(tick(1)), tick(1) + 16);
    assert!(!close.silence);
}

#[test]
fn a_ten_second_mark_stamps_without_a_silence() {
    let mark = RailMark::of(0, Some(tick(9) + 10), tick(10));
    assert!(mark.tick);
    assert!(!mark.silence);
    assert_eq!(mark.stamp, Some(10));
    let later = RailMark::of(0, Some(tick(10) + 1), tick(10) + 17);
    assert!(later.tick);
    assert_eq!(later.stamp, None);
}

#[test]
fn the_text_is_seven_cells_with_the_stamp_right_aligned() {
    let icons = &Icons::UNICODE;
    assert_eq!(RailMark::default().text(icons), "       ");
    let tick_only = RailMark {
        tick: true,
        ..RailMark::default()
    };
    assert_eq!(tick_only.text(icons), format!("     {} ", icons.rail_tick));
    let stamped = RailMark {
        tick: true,
        silence: true,
        stamp: Some(62),
    };
    assert_eq!(stamped.text(icons), format!(" 1:02{} ", icons.rail_break));
    let long = RailMark {
        tick: true,
        silence: false,
        stamp: Some(3_600 + 120),
    };
    assert_eq!(long.text(icons), format!(" 1h02{} ", icons.rail_tick));
    for mark in [tick_only, stamped, long] {
        assert_eq!(
            crate::render::width(&mark.text(icons)),
            RailMark::COLS,
            "{mark:?}"
        );
    }
}
