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
    graphics.finish(&mut Encoder::new(), &mut wire);
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
