use super::*;
use crate::layout::Rect;
use crate::pixel::PixelFormat;
use crate::pixel::{CellSize, PixelSurface};

/// The RGBA buffer as rows of glyphs by alpha: ` ` transparent, `.` `+`
/// `#` increasingly opaque.
fn ascii(surface: &PixelSurface) -> String {
    let width = usize::from(surface.width());
    surface
        .pixels()
        .chunks(4)
        .map(|px| match px[3] {
            0..=40 => ' ',
            41..=110 => '.',
            111..=180 => '+',
            _ => '#',
        })
        .collect::<Vec<char>>()
        .chunks(width)
        .map(|row| row.iter().collect::<String>())
        .collect::<Vec<String>>()
        .join("\n")
}

fn render(icon: Icon) -> PixelSurface {
    let mut surface = PixelSurface::with_alpha(Rect::new(0, 0, 2, 1), CellSize::new(8, 16));
    surface.paint(&icon);
    surface
}

#[test]
fn snap_check() {
    insta::assert_snapshot!(ascii(&render(Icon::Check)));
}

#[test]
fn snap_cross() {
    insta::assert_snapshot!(ascii(&render(Icon::Cross)));
}

#[test]
fn snap_dot() {
    insta::assert_snapshot!(ascii(&render(Icon::Dot)));
}

#[test]
fn snap_play() {
    insta::assert_snapshot!(ascii(&render(Icon::Play)));
}

#[test]
fn an_icon_is_transparent_around_the_glyph_and_carries_its_colour() {
    let surface = render(Icon::Dot);
    assert_eq!(surface.format(), PixelFormat::Rgba);
    assert_eq!(&surface.pixels()[..4], &[0, 0, 0, 0], "the corner is clear");
    let centre = (8 * 16 + 8) * 4;
    let px = &surface.pixels()[centre..centre + 4];
    assert_eq!(px[3], 255, "the centre is opaque");
    assert_eq!(&px[..3], &[122, 162, 247], "the dot's colour, straight");
}
