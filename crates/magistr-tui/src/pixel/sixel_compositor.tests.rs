use tiny_skia::{Color, PixmapMut};

use super::*;
use magistr_cockpit::pixel::CellSize;

/// The sixel images in `bytes`, DCS body by DCS body.
fn images(bytes: &[u8]) -> Vec<Vec<u8>> {
    let text = bytes.to_vec();
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(start) = text[at..]
        .windows(2)
        .position(|w| w == b"\x1bP")
        .map(|offset| at + offset)
    {
        let end = text[start..]
            .windows(2)
            .position(|w| w == b"\x1b\\")
            .map(|offset| start + offset)
            .expect("a DCS is terminated");
        found.push(text[start..end + 2].to_vec());
        at = end + 2;
    }
    found
}

/// Whether `needle` appears anywhere in `bytes`.
fn has(bytes: &[u8], needle: &[u8]) -> bool {
    bytes.windows(needle.len()).any(|window| window == needle)
}

fn surface(rect: Rect) -> PixelSurface {
    let mut surface = PixelSurface::new(rect, CellSize::new(2, 2));
    surface.paint(&|pixmap: &mut PixmapMut<'_>| pixmap.fill(Color::from_rgba8(1, 2, 3, 255)));
    surface
}

/// Show every surface, finish the frame, and return the bytes together
/// with whether a rect was vacated.
fn frame(
    compositor: &mut SixelCompositor,
    encoder: &mut Encoder,
    shows: &mut [(u32, &mut PixelSurface)],
) -> (Vec<u8>, bool) {
    for (id, surface) in shows.iter_mut() {
        compositor.show(WidgetId::new(*id), surface);
    }
    let mut out = Vec::new();
    let vacated = compositor.finish(encoder, &mut out);
    (out, vacated)
}

#[test]
fn the_first_frame_moves_the_cursor_and_emits_the_whole_image() {
    let mut compositor = SixelCompositor::new();
    let mut encoder = Encoder::new();
    let mut s = surface(Rect::new(3, 4, 5, 2));

    let (out, vacated) = frame(&mut compositor, &mut encoder, &mut [(7, &mut s)]);

    // Row 5, column 4 one-based, then DECSC, the image, DECRC.
    assert!(out.starts_with(b"\x1b[5;4H\x1b7\x1bP"), "{out:?}");
    assert!(out.ends_with(b"\x1b\\\x1b8"), "{out:?}");
    assert_eq!(images(&out).len(), 1);
    // 5 cells x 2 cells at 2x2 px.
    assert!(
        images(&out)[0].starts_with(b"\x1bP0;1;0q\"1;1;10;4"),
        "raster size"
    );
    assert!(!vacated, "nothing left the frame");
}

#[test]
fn an_unchanged_surface_costs_nothing() {
    let mut compositor = SixelCompositor::new();
    let mut encoder = Encoder::new();
    let mut s = surface(Rect::new(0, 0, 4, 2));
    frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);

    let (out, vacated) = frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);

    assert!(out.is_empty(), "{out:?}");
    assert!(!vacated);
}

#[test]
fn a_moved_surface_is_re_emitted_unchanged_and_vacates_its_old_rect() {
    let mut compositor = SixelCompositor::new();
    let mut encoder = Encoder::new();
    let mut s = surface(Rect::new(0, 0, 4, 2));
    let (first, _) = frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);

    s.set_rect(Rect::new(6, 3, 4, 2));
    let (out, vacated) = frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);

    assert!(out.starts_with(b"\x1b[4;7H"), "moved to the new cell");
    assert_eq!(
        images(&out),
        images(&first),
        "a move re-emits the cached image, it does not re-encode"
    );
    assert!(vacated, "the old rect's cells are stale");
}

#[test]
fn a_repainted_surface_is_encoded_again() {
    let mut compositor = SixelCompositor::new();
    let mut encoder = Encoder::new();
    let mut s = surface(Rect::new(0, 0, 4, 2));
    let (first, _) = frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);

    s.paint(&|pixmap: &mut PixmapMut<'_>| pixmap.fill(Color::from_rgba8(200, 100, 50, 255)));
    let (out, vacated) = frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);

    assert_eq!(images(&out).len(), 1);
    assert_ne!(
        images(&out),
        images(&first),
        "different pixels, different DCS"
    );
    assert!(!vacated, "the surface never left its rect");
}

#[test]
fn a_surface_that_leaves_the_frame_vacates_its_rect() {
    let mut compositor = SixelCompositor::new();
    let mut encoder = Encoder::new();
    let mut s = surface(Rect::new(0, 0, 4, 2));
    frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);

    let (out, vacated) = frame(&mut compositor, &mut encoder, &mut []);

    assert!(out.is_empty(), "there is no delete command to send");
    assert!(vacated, "the cells it covered have to be painted again");
}

#[test]
fn an_empty_rect_is_not_shown() {
    let mut compositor = SixelCompositor::new();
    let mut encoder = Encoder::new();
    let mut s = surface(Rect::new(2, 2, 0, 4));

    let (out, vacated) = frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);

    assert!(out.is_empty(), "{out:?}");
    assert!(!vacated);
}

#[test]
fn a_cleared_display_re_emits_without_re_encoding_and_vacates_nothing() {
    let mut compositor = SixelCompositor::new();
    let mut encoder = Encoder::new();
    let mut s = surface(Rect::new(0, 0, 4, 2));
    let (first, _) = frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);

    compositor.forget_placements();
    encoder.reset();
    let (out, vacated) = frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);

    assert_eq!(images(&out), images(&first));
    assert!(!vacated, "the clear already repainted the cells");
}

#[test]
fn release_writes_nothing_and_forgets_every_surface() {
    let mut compositor = SixelCompositor::new();
    let mut encoder = Encoder::new();
    let mut s = surface(Rect::new(0, 0, 4, 2));
    frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);

    let mut out = Vec::new();
    compositor.release(&mut out);
    assert!(out.is_empty(), "a sixel terminal stores nothing to delete");

    // The surface is clean, so only the forgotten slot can bring it back.
    encoder.reset();
    let (again, _) = frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);
    assert_eq!(images(&again).len(), 1, "shown again from scratch");
}

#[test]
fn two_surfaces_are_emitted_at_their_own_cells() {
    let mut compositor = SixelCompositor::new();
    let mut encoder = Encoder::new();
    let mut left = surface(Rect::new(0, 0, 4, 2));
    let mut right = surface(Rect::new(10, 6, 3, 1));

    let (out, _) = frame(
        &mut compositor,
        &mut encoder,
        &mut [(1, &mut left), (2, &mut right)],
    );

    assert_eq!(images(&out).len(), 2);
    assert!(has(&out, b"\x1b[1;1H"), "left at 1;1");
    assert!(has(&out, b"\x1b[7;11H"), "right at 7;11");
}
