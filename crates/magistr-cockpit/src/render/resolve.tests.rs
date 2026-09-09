use super::*;
use crate::theme::ThemeMode;

#[test]
fn the_default_colour_is_the_palettes_text_or_base() {
    for mode in [ThemeMode::Dark, ThemeMode::Light] {
        let palette = mode.palette();
        let text = palette.text;
        let base = palette.base;
        assert_eq!(
            resolve(Color::Default, &palette, true),
            [text.r, text.g, text.b]
        );
        assert_eq!(
            resolve(Color::Default, &palette, false),
            [base.r, base.g, base.b]
        );
    }
}

#[test]
fn truecolor_is_itself() {
    let palette = ThemeMode::Dark.palette();
    assert_eq!(resolve(Color::Rgb(1, 2, 3), &palette, true), [1, 2, 3]);
    assert_eq!(resolve(Color::Rgb(1, 2, 3), &palette, false), [1, 2, 3]);
}

#[test]
fn indexed_colours_follow_xterms_table() {
    let palette = ThemeMode::Dark.palette();
    let at = |n| resolve(Color::Indexed(n), &palette, true);
    assert_eq!(at(0), [0, 0, 0]);
    assert_eq!(at(1), [0xcd, 0, 0]);
    assert_eq!(at(9), [0xff, 0, 0]);
    assert_eq!(at(15), [0xff, 0xff, 0xff]);
    assert_eq!(at(16), [0, 0, 0]);
    assert_eq!(at(21), [0, 0, 255]);
    assert_eq!(at(196), [255, 0, 0]);
    assert_eq!(at(231), [255, 255, 255]);
    assert_eq!(at(232), [8, 8, 8]);
    assert_eq!(at(255), [238, 238, 238]);
}
