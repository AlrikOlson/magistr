use super::*;

#[test]
fn the_event_glyph_column_goes_and_the_separator_stays_readable() {
    assert_eq!(
        plain("▸ task started · wire the banner"),
        "task started - wire the banner"
    );
    assert_eq!(
        plain("✦ think 361 · open the iteration"),
        "think 361 - open the iteration"
    );
}

#[test]
fn box_drawing_blocks_and_nerd_font_icons_are_all_gone() {
    for glyph in [
        "─", "│", "╭", "▰", "▱", "★", "⚑", "◆", "⚠", "✓", "✗", "⇱", "\u{e0b0}",
    ] {
        assert_eq!(plain(glyph), "", "{glyph} is chrome, not content");
    }
}

#[test]
fn words_survive_intact_in_every_script() {
    assert_eq!(plain("cargo nextest run"), "cargo nextest run");
    assert_eq!(plain("naïve café — dash"), "naïve café — dash");
    assert_eq!(plain("日本語のテキスト"), "日本語のテキスト");
}

#[test]
fn what_a_removed_glyph_leaves_behind_is_squeezed_away() {
    assert_eq!(plain("  ▸   gates   green  "), "gates green");
    assert_eq!(plain("▸"), "");
    assert_eq!(plain(""), "");
}
