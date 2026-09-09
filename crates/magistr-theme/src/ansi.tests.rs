use super::*;
use crate::palette::{all_builtin_palettes, catppuccin_latte, catppuccin_mocha};

#[test]
fn the_six_hues_are_the_palettes_own_in_sgr_order() {
    let p = catppuccin_mocha();
    let ansi = ansi16(&p);
    assert_eq!(ansi[1], p.red);
    assert_eq!(ansi[2], p.green);
    assert_eq!(ansi[3], p.yellow);
    assert_eq!(ansi[4], p.blue);
    assert_eq!(ansi[5], p.mauve, "magenta is the tool accent");
    assert_eq!(ansi[6], p.sky, "cyan is the running accent");
}

#[test]
fn the_bright_hues_are_the_same_six_and_the_greys_climb_the_surfaces() {
    for p in [catppuccin_mocha(), catppuccin_latte()] {
        let ansi = ansi16(&p);
        assert_eq!(ansi[9..15], ansi[1..7], "{}: bright hues", p.name);
        assert_eq!(ansi[0], p.surface1, "{}: black", p.name);
        assert_eq!(ansi[8], p.surface2, "{}: bright black", p.name);
        assert_eq!(ansi[7], p.subtext1, "{}: white", p.name);
        assert_eq!(ansi[15], p.text, "{}: bright white", p.name);
    }
}

#[test]
fn every_builtin_palette_answers_sixteen_and_black_is_darker_than_white_by_medium() {
    use crate::math::relative_luminance;
    for p in all_builtin_palettes() {
        let ansi = ansi16(&p);
        let black = relative_luminance(ansi[0]);
        let white = relative_luminance(ansi[15]);
        let base = relative_luminance(p.base);
        // On a dark palette black sits near the base and white far
        // from it; on a light one the other way round — either way the
        // pair straddles the text's own direction.
        if base < 0.5 {
            assert!(black < white, "{}: dark palette", p.name);
        } else {
            assert!(black > white, "{}: light palette", p.name);
        }
    }
}
