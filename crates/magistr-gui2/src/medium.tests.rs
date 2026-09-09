use super::*;

#[test]
fn each_medium_names_its_capture_and_its_palette() {
    assert_eq!(Medium::Night.prefix(), "night");
    assert_eq!(Medium::Night.mode(), ThemeMode::Dark);
    assert_eq!(Medium::Day.prefix(), "day");
    assert_eq!(Medium::Day.mode(), ThemeMode::Light);
}

#[test]
fn a_mask_word_round_trips_and_anything_else_is_refused() {
    for medium in Medium::ALL {
        assert_eq!(Medium::parse(medium.prefix()), Some(medium));
        assert_eq!(medium.to_string(), medium.prefix());
    }
    assert_eq!(Medium::parse("dusk"), None);
    assert_eq!(Medium::parse("Night"), None);
}
