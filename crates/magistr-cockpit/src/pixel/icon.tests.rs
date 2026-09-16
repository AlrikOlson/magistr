use super::*;
use crate::golden::Golden;
use crate::layout::Rect;
use crate::pixel::PixelFormat;
use crate::pixel::{CellSize, PixelSurface};
use crate::render::Color as CellColor;
use crate::theme::{Pen, ThemeMode};

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
    surface.paint(&|pixmap: &mut PixmapMut<'_>| icon.draw(pixmap, Color::WHITE));
    surface
}

#[test]
fn snap_done() {
    insta::assert_snapshot!(ascii(&render(Icon::Done)));
}

#[test]
fn snap_failed() {
    insta::assert_snapshot!(ascii(&render(Icon::Failed)));
}

#[test]
fn snap_ready() {
    insta::assert_snapshot!(ascii(&render(Icon::Ready)));
}

#[test]
fn snap_in_progress() {
    insta::assert_snapshot!(ascii(&render(Icon::InProgress)));
}

#[test]
fn an_icon_is_transparent_around_the_glyph_and_carries_its_ink() {
    let mut surface = PixelSurface::with_alpha(Rect::new(0, 0, 2, 1), CellSize::new(8, 16));
    let ink = Color::from_rgba8(122, 162, 247, 255);
    surface.paint(&|pixmap: &mut PixmapMut<'_>| Icon::Ready.draw(pixmap, ink));
    assert_eq!(surface.format(), PixelFormat::Rgba);
    assert_eq!(&surface.pixels()[..4], &[0, 0, 0, 0], "the corner is clear");
    let centre = (8 * 16 + 8) * 4;
    let px = &surface.pixels()[centre..centre + 4];
    assert_eq!(px[3], 255, "the centre is opaque");
    assert_eq!(&px[..3], &[122, 162, 247], "the ink, straight");
}

#[test]
fn every_glyph_draws_something_inside_its_square_and_nothing_outside() {
    for icon in Icon::ALL {
        let surface = render(icon);
        let width = usize::from(surface.width());
        let mut inked = 0;
        for (index, px) in surface.pixels().chunks(4).enumerate() {
            if px[3] == 0 {
                continue;
            }
            inked += 1;
            let (x, y) = (index % width, index / width);
            // The glyph square is the 16px side centred in the 16x16
            // surface: every inked pixel is on it.
            assert!(x < 16 && y < 16, "{}: ink at ({x}, {y})", icon.name());
        }
        assert!(inked > 8, "{} draws {inked} pixels", icon.name());
    }
}

#[test]
fn the_spinner_turns_a_quarter_a_step_and_repeats_every_four() {
    let frame = |step: u8| render(Icon::Spinner(step)).pixels().to_vec();
    assert_ne!(frame(0), frame(1));
    assert_ne!(frame(1), frame(2));
    assert_eq!(frame(0), frame(4));
}

#[test]
fn fold_check_and_gate_read_from_the_vocabulary() {
    assert_eq!(Icon::fold(true), Icon::FoldOpen);
    assert_eq!(Icon::fold(false), Icon::FoldClosed);
    assert_eq!(Icon::check(true), Icon::Done);
    assert_eq!(Icon::check(false), Icon::Failed);
    assert_eq!(Icon::gate(None), Icon::Waiting);
    assert_eq!(Icon::gate(Some(true)), Icon::Done);
    assert_eq!(Icon::gate(Some(false)), Icon::Failed);
}

#[test]
fn every_name_is_distinct_and_a_file_stem() {
    let mut names: Vec<&str> = Icon::ALL.iter().map(|icon| icon.name()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), Icon::ALL.len());
    for name in names {
        assert!(
            name.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
            "{name}"
        );
    }
}

/// One golden per glyph per theme, drawn in the pen's ink for the
/// glyph over the theme's base — the set as it appears on screen,
/// reviewed as pictures. A mismatch writes a candidate beside the
/// golden; an intended change is blessed explicitly.
#[test]
fn golden_every_icon_in_both_themes() {
    let goldens =
        Golden::new(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/icons"));
    let mut failures = Vec::new();
    for mode in [ThemeMode::Dark, ThemeMode::Light] {
        let palette = mode.palette();
        let pen = Pen::of(&palette);
        let base = (palette.base.r, palette.base.g, palette.base.b);
        for icon in Icon::ALL {
            let ink = match pen.ink(icon) {
                CellColor::Rgb(r, g, b) => Color::from_rgba8(r, g, b, 255),
                other => panic!("{}: the ink is {other:?}, not RGB", icon.name()),
            };
            // Two cells of a 10x20 grid: the glyph is drawn at four
            // times its size so its weight and shape can be read.
            let mut surface = PixelSurface::new(Rect::new(0, 0, 4, 2), CellSize::new(10, 20));
            surface.paint(&|pixmap: &mut PixmapMut<'_>| {
                pixmap.fill(Color::from_rgba8(base.0, base.1, base.2, 255));
                let mut glyph =
                    tiny_skia::Pixmap::new(pixmap.width(), pixmap.height()).expect("a pixmap");
                icon.draw(&mut glyph.as_mut(), ink);
                pixmap.draw_pixmap(
                    0,
                    0,
                    glyph.as_ref(),
                    &tiny_skia::PixmapPaint::default(),
                    Transform::identity(),
                    None,
                );
            });
            let pixmap = tiny_skia::Pixmap::from_vec(
                surface.pixels().to_vec(),
                tiny_skia::IntSize::from_wh(
                    u32::from(surface.width()),
                    u32::from(surface.height()),
                )
                .expect("size"),
            )
            .expect("pixmap");
            let name = format!("{}-{}", icon.name(), mode.as_str());
            if let Err(error) = goldens.check(&name, &pixmap) {
                failures.push(format!("{name}: {error}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
