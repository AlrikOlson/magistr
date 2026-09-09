use super::*;

#[test]
fn every_style_round_trips_through_its_sgr_code() {
    for style in Underline::ALL {
        assert_eq!(
            Underline::from_code(style.code()),
            Some(style),
            "{style:?} round-trips"
        );
    }
}

#[test]
fn the_codes_are_kittys_four_x_sub_parameters() {
    assert_eq!(Underline::Single.code(), 1);
    assert_eq!(Underline::Double.code(), 2);
    assert_eq!(Underline::Curly.code(), 3);
    assert_eq!(Underline::Dotted.code(), 4);
    assert_eq!(Underline::Dashed.code(), 5);
}

#[test]
fn zero_and_anything_past_dashed_name_no_style() {
    assert_eq!(Underline::from_code(0), None);
    for code in 6..=u8::MAX {
        assert_eq!(Underline::from_code(code), None, "{code}");
    }
}

#[test]
fn the_default_is_the_plain_rule() {
    assert_eq!(Underline::default(), Underline::Single);
}
