use super::*;
use crate::cockpit::Size;
use crate::theme::{Theme, ThemeMode};
use magistr_theme::math::contrast_ratio;

#[test]
fn all_text_is_legible_on_all_finished_surfaces() {
    for mode in [ThemeMode::Dark, ThemeMode::Light] {
        let model = Model::new(Size::new(144, 50)).with_theme(Theme::forced(mode));
        let style = FlightStyle::for_model(&model);
        for fg in [
            style.ink,
            style.muted,
            style.accent,
            style.good,
            style.lane(1),
            style.lane(2),
        ] {
            for bg in [
                style.root,
                style.panel,
                style.header,
                style.raised,
                style.selected,
            ] {
                let Color::Rgb(r, g, b) = contrast(fg, bg) else {
                    panic!("truecolor");
                };
                let Color::Rgb(br, bg, bb) = bg else {
                    panic!("truecolor");
                };
                assert!(
                    contrast_ratio(ThemeColor::new(r, g, b), ThemeColor::new(br, bg, bb)) >= 4.49
                );
            }
        }
    }
}
