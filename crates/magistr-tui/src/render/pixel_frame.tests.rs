use super::*;
use crate::graphics::{Passthrough, Protocol};
use crate::pixel::{Compression, IdSpace};
use crate::render::Encoder;

fn send(
    frame: &mut PixelFrame,
    graphics: &mut Backend,
    color: tiny_skia::Color,
    edit: bool,
) -> Vec<u8> {
    let mut ground = Pixmap::new(100, 160).expect("frame");
    ground.fill(color);
    if edit {
        ground.data_mut()[0] = 255;
    }
    let ink = Pixmap::new(100, 160).expect("ink");
    frame.begin(ground);
    frame.finish(&ink, 10, 8, CellSize::DEFAULT, graphics);
    let mut wire = Vec::new();
    assert!(graphics.prepare().is_empty());
    graphics.finish(&mut Encoder::new(), &mut wire, &[]);
    wire
}

#[test]
fn unchanged_pixels_cost_no_graphics_bytes_and_one_edit_resends_one_strip() {
    let mut frame = PixelFrame::default();
    let mut graphics = Backend::new(
        Protocol::Kitty(Passthrough::Direct),
        Compression::None,
        IdSpace::FIRST,
    );
    let first = send(&mut frame, &mut graphics, tiny_skia::Color::BLACK, false);
    assert!(!first.is_empty());
    assert!(send(&mut frame, &mut graphics, tiny_skia::Color::BLACK, false).is_empty());
    let edited = send(&mut frame, &mut graphics, tiny_skia::Color::BLACK, true);
    assert!(edited.len() < first.len());
    assert_eq!(
        edited
            .windows(6)
            .filter(|bytes| *bytes == b"\x1b_Ga=p")
            .count(),
        1
    );
}

#[test]
fn physical_placement_and_overlay_do_not_round_to_terminal_cells() {
    let mut frame = PixelFrame::default();
    let mut ground = Pixmap::new(100, 80).expect("frame");
    ground.fill(tiny_skia::Color::BLACK);
    frame.begin(ground);
    let mut surface = PixelSurface::new(Rect::new(0, 0, 20, 12), CellSize::new(1, 1));
    surface.paint(&|pixels: &mut tiny_skia::PixmapMut<'_>| pixels.fill(tiny_skia::Color::WHITE));
    frame.blend_at(
        &mut surface,
        PxRect::new(13.0, 23.0, 20.0, 12.0),
        Some(PxRect::new(21.0, 24.0, 5.0, 6.0)),
    );
    let canvas = frame.canvas.as_ref().expect("canvas");
    let at = |x: usize, y: usize| &canvas.data()[(y * 100 + x) * 4..(y * 100 + x + 1) * 4];
    assert_eq!(at(12, 23), [0, 0, 0, 255]);
    assert_eq!(at(13, 23), [255, 255, 255, 255]);
    assert_eq!(at(20, 25), [255, 255, 255, 255]);
    assert_eq!(at(21, 25), [0, 0, 0, 255]);
    assert_eq!(at(26, 25), [255, 255, 255, 255]);
    assert_eq!(at(33, 23), [0, 0, 0, 255]);
}

#[test]
fn an_empty_viewport_does_not_index_a_nonexistent_strip() {
    let mut frame = PixelFrame::default();
    let mut graphics = Backend::new(Protocol::Sixel, Compression::None, IdSpace::FIRST);
    frame.begin(Pixmap::new(1, 1).expect("empty frame"));
    frame.finish(
        &Pixmap::new(1, 1).expect("ink"),
        0,
        0,
        CellSize::DEFAULT,
        &mut graphics,
    );
    assert!(frame.tiles.is_empty());
}
