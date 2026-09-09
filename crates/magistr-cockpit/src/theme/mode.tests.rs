use super::*;

#[test]
fn parse_accepts_dark_and_light_only() {
    assert_eq!(ThemeMode::parse("dark"), Some(ThemeMode::Dark));
    assert_eq!(ThemeMode::parse("light"), Some(ThemeMode::Light));
    assert_eq!(ThemeMode::parse(" LIGHT "), Some(ThemeMode::Light));
    assert_eq!(ThemeMode::parse("Dark"), Some(ThemeMode::Dark));
    assert_eq!(ThemeMode::parse(""), None);
    assert_eq!(ThemeMode::parse("auto"), None);
    assert_eq!(ThemeMode::parse("latte"), None);
    assert_eq!(ThemeMode::parse("catppuccin-mocha"), None);
}

#[test]
fn a_bright_background_is_light_and_a_dim_one_dark() {
    // The two Catppuccin bases themselves.
    assert_eq!(
        ThemeMode::for_background(Color::Rgb(30, 30, 46)),
        ThemeMode::Dark
    );
    assert_eq!(
        ThemeMode::for_background(Color::Rgb(239, 241, 245)),
        ThemeMode::Light
    );
    assert_eq!(
        ThemeMode::for_background(Color::Rgb(0, 0, 0)),
        ThemeMode::Dark
    );
    assert_eq!(
        ThemeMode::for_background(Color::Rgb(255, 255, 255)),
        ThemeMode::Light
    );
    // Solarized light (#fdf6e3) and dark (#002b36).
    assert_eq!(
        ThemeMode::for_background(Color::Rgb(253, 246, 227)),
        ThemeMode::Light
    );
    assert_eq!(
        ThemeMode::for_background(Color::Rgb(0, 43, 54)),
        ThemeMode::Dark
    );
    // Mid grey: relative luminance 0.216, well under the 0.5 threshold.
    assert_eq!(
        ThemeMode::for_background(Color::Rgb(128, 128, 128)),
        ThemeMode::Dark
    );
}

#[test]
fn a_background_that_is_not_rgb_gives_no_evidence_of_light() {
    assert_eq!(ThemeMode::for_background(Color::Default), ThemeMode::Dark);
    assert_eq!(
        ThemeMode::for_background(Color::Indexed(15)),
        ThemeMode::Dark
    );
    assert_eq!(
        ThemeMode::for_background(Color::Indexed(0)),
        ThemeMode::Dark
    );
}

#[test]
fn the_palettes_are_catppuccin_and_agree_with_their_mode() {
    let dark = ThemeMode::Dark.palette();
    let light = ThemeMode::Light.palette();
    assert_eq!(dark.name, "catppuccin-mocha");
    assert_eq!(light.name, "catppuccin-latte");
    for (mode, palette) in [(ThemeMode::Dark, dark), (ThemeMode::Light, light)] {
        let base = Color::Rgb(palette.base.r, palette.base.g, palette.base.b);
        assert_eq!(ThemeMode::for_background(base), mode, "{}", palette.name);
    }
}

#[test]
fn display_is_the_env_vocabulary() {
    assert_eq!(ThemeMode::Dark.to_string(), "dark");
    assert_eq!(ThemeMode::Light.to_string(), "light");
    for mode in [ThemeMode::Dark, ThemeMode::Light] {
        assert_eq!(ThemeMode::parse(mode.as_str()), Some(mode));
    }
    assert_eq!(ThemeMode::default(), ThemeMode::Dark);
}
