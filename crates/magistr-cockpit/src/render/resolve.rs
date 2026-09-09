//! A terminal colour as the pixels it paints, through the palette.

use magistr_theme::Palette;

use crate::render::Color;

/// xterm's default 16 ANSI colours — what a terminal shows for
/// `Color::Indexed(0..=15)` before any theme touches it.
const ANSI: [[u8; 3]; 16] = [
    [0x00, 0x00, 0x00],
    [0xcd, 0x00, 0x00],
    [0x00, 0xcd, 0x00],
    [0xcd, 0xcd, 0x00],
    [0x00, 0x00, 0xee],
    [0xcd, 0x00, 0xcd],
    [0x00, 0xcd, 0xcd],
    [0xe5, 0xe5, 0xe5],
    [0x7f, 0x7f, 0x7f],
    [0xff, 0x00, 0x00],
    [0x00, 0xff, 0x00],
    [0xff, 0xff, 0x00],
    [0x5c, 0x5c, 0xff],
    [0xff, 0x00, 0xff],
    [0x00, 0xff, 0xff],
    [0xff, 0xff, 0xff],
];

/// The six levels of xterm's 6×6×6 colour cube (indices 16..=231).
const CUBE: [u8; 6] = [0, 95, 135, 175, 215, 255];

/// `color` as opaque RGB. The terminal's default is the palette's `text`
/// for a foreground and its `base` for a background — the cockpit fills
/// the screen in those, so a screenshot shows the theme, not a terminal's
/// own defaults. An indexed colour is xterm's table (16 ANSI colours,
/// the 6×6×6 cube, 24 greys); truecolor is itself.
#[must_use]
pub fn resolve(color: Color, palette: &Palette, foreground: bool) -> [u8; 3] {
    match color {
        Color::Default => {
            let c = if foreground {
                palette.text
            } else {
                palette.base
            };
            [c.r, c.g, c.b]
        }
        Color::Rgb(r, g, b) => [r, g, b],
        Color::Indexed(n) => indexed(n),
    }
}

/// xterm's 256-colour table.
fn indexed(n: u8) -> [u8; 3] {
    match n {
        0..=15 => ANSI[usize::from(n)],
        16..=231 => {
            let i = n - 16;
            [
                CUBE[usize::from(i / 36)],
                CUBE[usize::from((i % 36) / 6)],
                CUBE[usize::from(i % 6)],
            ]
        }
        232..=255 => {
            let v = 8 + (n - 232) * 10;
            [v, v, v]
        }
    }
}

#[cfg(test)]
#[path = "resolve.tests.rs"]
mod tests;
