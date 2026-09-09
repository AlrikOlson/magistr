use super::*;
use crate::color::ThemeColor;

#[test]
fn luminance_black() {
    let lum = relative_luminance(ThemeColor::new(0, 0, 0));
    assert!((lum - 0.0).abs() < 0.001);
}

#[test]
fn luminance_white() {
    let lum = relative_luminance(ThemeColor::new(255, 255, 255));
    assert!((lum - 1.0).abs() < 0.001);
}

#[test]
fn contrast_black_white() {
    let ratio = contrast_ratio(ThemeColor::new(0, 0, 0), ThemeColor::new(255, 255, 255));
    assert!((ratio - 21.0).abs() < 0.1);
}

#[test]
fn contrast_same_color() {
    let c = ThemeColor::new(128, 128, 128);
    let ratio = contrast_ratio(c, c);
    assert!((ratio - 1.0).abs() < 0.001);
}

#[test]
fn validate_contrast_passes() {
    let result = validate_contrast(
        ThemeColor::new(255, 255, 255),
        ThemeColor::new(0, 0, 0),
        4.5,
        "test",
    );
    assert!(result.is_none());
}

#[test]
fn validate_contrast_fails() {
    let result = validate_contrast(
        ThemeColor::new(128, 128, 128),
        ThemeColor::new(120, 120, 120),
        4.5,
        "gray on gray",
    );
    assert!(result.is_some());
    let v = result.unwrap();
    assert_eq!(v.pair, "gray on gray");
    assert!(v.ratio < 4.5);
}

#[test]
fn darken_by_zero() {
    let c = ThemeColor::new(100, 200, 50);
    assert_eq!(darken(c, 0.0), c);
}

#[test]
fn darken_to_black() {
    let c = ThemeColor::new(100, 200, 50);
    assert_eq!(darken(c, 1.0), ThemeColor::new(0, 0, 0));
}

#[test]
fn lighten_by_zero() {
    let c = ThemeColor::new(100, 200, 50);
    assert_eq!(lighten(c, 0.0), c);
}

#[test]
fn lighten_to_white() {
    let c = ThemeColor::new(100, 200, 50);
    assert_eq!(lighten(c, 1.0), ThemeColor::new(255, 255, 255));
}

#[test]
fn lerp_endpoints() {
    let a = ThemeColor::new(0, 0, 0);
    let b = ThemeColor::new(255, 255, 255);
    assert_eq!(lerp(a, b, 0.0), a);
    assert_eq!(lerp(a, b, 1.0), b);
}

#[test]
fn lerp_midpoint() {
    let a = ThemeColor::new(0, 0, 0);
    let b = ThemeColor::new(200, 100, 50);
    let mid = lerp(a, b, 0.5);
    assert_eq!(mid, ThemeColor::new(100, 50, 25));
}

#[test]
fn darken_clamps_above_one() {
    let c = ThemeColor::new(100, 100, 100);
    assert_eq!(darken(c, 2.0), ThemeColor::new(0, 0, 0));
}

#[test]
fn lighten_clamps_above_one() {
    let c = ThemeColor::new(100, 100, 100);
    assert_eq!(lighten(c, 2.0), ThemeColor::new(255, 255, 255));
}

#[test]
fn ensure_contrast_leaves_a_passing_pair_alone() {
    let fg = ThemeColor::new(0xcd, 0xd6, 0xf4);
    let bg = ThemeColor::new(0x1e, 0x1e, 0x2e);
    assert_eq!(ensure_contrast(fg, bg, 3.0), fg);
}

#[test]
fn ensure_contrast_lightens_against_a_dark_background() {
    // A dim blue on a near-black base: too close to read at 3:1.
    let fg = ThemeColor::new(0x20, 0x30, 0x60);
    let bg = ThemeColor::new(0x1e, 0x1e, 0x2e);
    let fixed = ensure_contrast(fg, bg, 3.0);
    assert!(contrast_ratio(fixed, bg) >= 3.0, "{fixed} on {bg}");
    assert!(
        relative_luminance(fixed) > relative_luminance(fg),
        "should move away from the dark background"
    );
}

#[test]
fn ensure_contrast_darkens_against_a_light_background() {
    // Catppuccin Latte's peach on its base: 2.64:1 measured.
    let fg = ThemeColor::new(0xfe, 0x64, 0x0b);
    let bg = ThemeColor::new(0xef, 0xf1, 0xf5);
    let fixed = ensure_contrast(fg, bg, 3.0);
    assert!(contrast_ratio(fixed, bg) >= 3.0, "{fixed} on {bg}");
    assert!(
        relative_luminance(fixed) < relative_luminance(fg),
        "should move away from the light background"
    );
}

#[test]
fn ensure_contrast_gives_up_at_the_extreme() {
    // Nothing clears 21:1 against mid grey — the result is the extreme,
    // not a panic or an endless walk.
    let grey = ThemeColor::new(0x80, 0x80, 0x80);
    let fixed = ensure_contrast(grey, grey, 21.0);
    assert_eq!(fixed, ThemeColor::new(0, 0, 0));
}

#[test]
fn ensure_contrast_holds_for_every_builtin_accent() {
    for palette in crate::all_builtin_palettes() {
        for (name, accent) in [
            ("blue", palette.blue),
            ("sky", palette.sky),
            ("green", palette.green),
            ("red", palette.red),
            ("yellow", palette.yellow),
            ("peach", palette.peach),
            ("mauve", palette.mauve),
            ("lavender", palette.lavender),
        ] {
            for (bg_name, bg) in [
                ("base", palette.base),
                ("surface0", palette.surface0),
                ("surface1", palette.surface1),
            ] {
                let fixed = ensure_contrast(accent, bg, 3.0);
                let ratio = contrast_ratio(fixed, bg);
                assert!(
                    ratio >= 3.0,
                    "{} {name} on {bg_name}: {ratio:.2}",
                    palette.name
                );
            }
        }
    }
}
