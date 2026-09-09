use super::*;

const MOCHA: Color = Color::Rgb(30, 30, 46);
const LATTE: Color = Color::Rgb(239, 241, 245);

fn from_env(vars: &[(&str, &str)]) -> Theme {
    Theme::from_lookup(|name| {
        vars.iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).to_owned())
    })
}

#[test]
fn nothing_known_is_dark_and_not_forced() {
    let theme = Theme::detect(None, None);
    assert_eq!(theme.mode(), ThemeMode::Dark);
    assert!(!theme.is_forced());
    assert_eq!(theme, Theme::default());
}

#[test]
fn the_reply_decides_when_the_environment_says_nothing() {
    let light = Theme::detect(Some(LATTE), None);
    assert_eq!(light.mode(), ThemeMode::Light);
    assert!(!light.is_forced());
    let dark = Theme::detect(Some(MOCHA), None);
    assert_eq!(dark.mode(), ThemeMode::Dark);
    assert!(!dark.is_forced());
    // An unparsable value is as good as none.
    let junk = Theme::detect(Some(LATTE), Some("auto"));
    assert_eq!(junk.mode(), ThemeMode::Light);
    assert!(!junk.is_forced());
}

#[test]
fn the_environment_wins_over_the_reply() {
    let forced_dark = Theme::detect(Some(LATTE), Some("dark"));
    assert_eq!(forced_dark.mode(), ThemeMode::Dark);
    assert!(forced_dark.is_forced());
    let forced_light = Theme::detect(Some(MOCHA), Some(" Light"));
    assert_eq!(forced_light.mode(), ThemeMode::Light);
    assert!(forced_light.is_forced());
    assert_eq!(
        Theme::detect(None, Some("light")),
        Theme::forced(ThemeMode::Light)
    );
}

#[test]
fn observing_a_reply_moves_a_detected_theme_and_leaves_a_forced_one() {
    let detected = Theme::detect(None, None).observe(LATTE);
    assert_eq!(detected.mode(), ThemeMode::Light);
    assert!(!detected.is_forced());
    assert_eq!(detected.observe(MOCHA).mode(), ThemeMode::Dark);

    let forced = Theme::forced(ThemeMode::Dark).observe(LATTE);
    assert_eq!(forced.mode(), ThemeMode::Dark);
    assert!(forced.is_forced());
}

#[test]
fn the_palette_follows_the_mode() {
    assert_eq!(
        Theme::detect(None, Some("light")).palette().name,
        "catppuccin-latte"
    );
    assert_eq!(
        Theme::detect(Some(MOCHA), None).palette().name,
        "catppuccin-mocha"
    );
}

#[test]
fn an_empty_environment_uses_the_complete_default_theme() {
    assert_eq!(from_env(&[]), Theme::default());
}

#[test]
fn every_theme_override_is_read_from_the_supplied_environment() {
    assert_eq!(
        from_env(&[
            ("MAGISTR_THEME", "light"),
            ("MAGISTR_ICONS", "unicode"),
            ("TERM_PROGRAM", "Ghostty"),
            ("MAGISTR_CVD", "protanopia"),
            ("MAGISTR_MOTION", "reduced"),
        ]),
        Theme::forced(ThemeMode::Light)
            .with_icons(IconMode::Unicode)
            .with_cvd(CvdMode::from_name("protanopia"))
            .with_motion(MotionMode::Reduced),
    );
}

#[test]
fn terminal_icons_and_each_motion_convention_are_read_when_not_overridden() {
    for key in [
        "REDUCE_MOTION",
        "NO_ANIMATIONS",
        "MAGISTR_TUI_SCREEN_READER",
    ] {
        assert_eq!(
            from_env(&[("TERM_PROGRAM", "WezTerm"), (key, "1")]),
            Theme::default()
                .with_icons(IconMode::Nerd)
                .with_motion(MotionMode::Reduced),
            "{key}",
        );
        assert_eq!(from_env(&[(key, "0")]), Theme::default(), "{key}");
    }
}

#[test]
fn explicit_full_motion_wins_over_inherited_reduce_conventions() {
    assert_eq!(
        from_env(&[
            ("MAGISTR_MOTION", "full"),
            ("REDUCE_MOTION", "1"),
            ("NO_ANIMATIONS", "1"),
            ("MAGISTR_TUI_SCREEN_READER", "1"),
        ]),
        Theme::default(),
    );
}

#[test]
fn invalid_theme_inputs_use_their_defaults() {
    assert_eq!(
        from_env(&[
            ("MAGISTR_THEME", "unknown"),
            ("MAGISTR_ICONS", "unknown"),
            ("TERM_PROGRAM", "unknown"),
            ("MAGISTR_CVD", "unknown"),
            ("MAGISTR_MOTION", "unknown"),
        ]),
        Theme::default(),
    );
}

#[test]
fn motion_rides_the_theme_and_survives_the_terminals_reply() {
    let theme = Theme::detect(None, None).with_motion(MotionMode::Reduced);
    assert_eq!(theme.motion(), MotionMode::Reduced);
    assert_eq!(theme.observe(LATTE).motion(), MotionMode::Reduced);
    assert_eq!(
        Theme::default().motion(),
        MotionMode::Full,
        "the full layer is the default"
    );
}
