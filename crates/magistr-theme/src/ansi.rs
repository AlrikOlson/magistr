//! A palette's sixteen ANSI colours — what a program's own `SGR 30–37`
//! and `90–97` paint as on a surface themed by that palette.

use crate::color::ThemeColor;
use crate::palette::Palette;

/// The sixteen ANSI colours `palette` answers, in SGR order: black,
/// red, green, yellow, blue, magenta, cyan, white, then the same eight
/// bright.
///
/// Catppuccin's own terminal mapping, which every palette here follows
/// since every palette has the same slots: black is the border surface
/// and bright black the one above it, white is the tertiary text and
/// bright white the text itself, and the six hues are the palette's
/// own — magenta the tool accent (mauve) and cyan the running accent
/// (sky), since the palette carries no pink or teal. The bright hues
/// are the same six: Catppuccin draws them identically, and a second
/// set of accents would be a second palette.
///
/// This is the one place a themed surface lets hue be *data* rather
/// than a state code (`docs/architecture/desktop.md#terminal-pane`): a program's red
/// stays a red, but the palette's red.
#[must_use]
pub fn ansi16(palette: &Palette) -> [ThemeColor; 16] {
    let hues = [
        palette.red,
        palette.green,
        palette.yellow,
        palette.blue,
        palette.mauve,
        palette.sky,
    ];
    [
        palette.surface1,
        hues[0],
        hues[1],
        hues[2],
        hues[3],
        hues[4],
        hues[5],
        palette.subtext1,
        palette.surface2,
        hues[0],
        hues[1],
        hues[2],
        hues[3],
        hues[4],
        hues[5],
        palette.text,
    ]
}

#[cfg(test)]
#[path = "ansi.tests.rs"]
mod tests;
