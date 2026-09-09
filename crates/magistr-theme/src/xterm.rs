//! The 256-colour table's upper 240 entries — xterm's own, which no
//! palette re-themes.

use crate::color::ThemeColor;

/// Colour `n` of the 256-colour table above the sixteen ANSI ones:
/// the 6×6×6 cube at `16..=231` on xterm's own levels
/// (0 · 95 · 135 · 175 · 215 · 255) and the 24 greys at `232..=255`
/// from 8 in steps of 10. A palette answers the first sixteen
/// ([`ansi16`](crate::ansi16)); these are the program's own numbers,
/// the same on every medium, so both painters read one table.
///
/// An index under 16 is answered as the cube's first entry — the
/// caller is expected to have taken the ANSI branch first.
#[must_use]
pub fn xterm256(n: u8) -> ThemeColor {
    const CUBE: [u8; 6] = [0, 95, 135, 175, 215, 255];
    if n >= 232 {
        let v = 8 + (n - 232) * 10;
        return ThemeColor::new(v, v, v);
    }
    let i = n.saturating_sub(16);
    ThemeColor::new(
        CUBE[usize::from(i / 36)],
        CUBE[usize::from((i % 36) / 6)],
        CUBE[usize::from(i % 6)],
    )
}

#[cfg(test)]
#[path = "xterm.tests.rs"]
mod tests;
