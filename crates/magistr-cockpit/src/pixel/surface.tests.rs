use tiny_skia::{Color, PixmapMut};

use super::*;
use crate::layout::Rect;
use crate::pixel::PixelFormat;

fn cell() -> CellSize {
    CellSize::new(4, 8)
}

#[test]
fn a_surface_is_its_rect_times_the_cell_size_and_starts_dirty() {
    let surface = PixelSurface::new(Rect::new(2, 3, 5, 2), cell());
    assert_eq!(surface.rect(), Rect::new(2, 3, 5, 2));
    assert_eq!(surface.cell_size(), cell());
    assert_eq!((surface.width(), surface.height()), (20, 16));
    assert_eq!(surface.pixels().len(), 20 * 16 * 4);
    assert!(surface.is_dirty(), "nothing has been transmitted yet");
    assert_eq!(surface.format(), PixelFormat::Rgb);
}

#[test]
fn painting_marks_the_surface_dirty_and_changes_the_pixels() {
    let mut surface = PixelSurface::new(Rect::new(0, 0, 1, 1), cell());
    surface.clean();
    assert!(!surface.is_dirty());
    surface.paint(&|pixmap: &mut PixmapMut<'_>| {
        pixmap.fill(Color::from_rgba8(10, 20, 30, 255));
    });
    assert!(surface.is_dirty());
    assert_eq!(&surface.pixels()[..4], &[10, 20, 30, 255]);
}

#[test]
fn an_opaque_surface_packs_three_bytes_per_pixel() {
    let mut surface = PixelSurface::new(Rect::new(0, 0, 1, 1), cell());
    surface.paint(&|pixmap: &mut PixmapMut<'_>| {
        pixmap.fill(Color::from_rgba8(10, 20, 30, 255));
    });
    let mut out = vec![7];
    assert_eq!(surface.pack(&mut out), PixelFormat::Rgb);
    assert_eq!(out.len(), 4 * 8 * 3);
    assert_eq!(&out[..6], &[10, 20, 30, 10, 20, 30]);
}

#[test]
fn an_alpha_surface_packs_straight_alpha() {
    let mut surface = PixelSurface::with_alpha(Rect::new(0, 0, 1, 1), cell());
    assert_eq!(surface.format(), PixelFormat::Rgba);
    surface.paint(&|pixmap: &mut PixmapMut<'_>| {
        pixmap.fill(Color::from_rgba8(255, 0, 0, 128));
    });
    // tiny-skia stores premultiplied: red is halved in the pixmap…
    assert_eq!(&surface.pixels()[..4], &[128, 0, 0, 128]);
    let mut out = Vec::new();
    assert_eq!(surface.pack(&mut out), PixelFormat::Rgba);
    assert_eq!(out.len(), 4 * 8 * 4);
    // …and demultiplied on the wire (kitty f=32 is straight alpha).
    assert_eq!(&out[..4], &[255, 0, 0, 128]);
}

#[test]
fn moving_keeps_the_pixels_and_resizing_reallocates_dirty() {
    let mut surface = PixelSurface::new(Rect::new(0, 0, 2, 1), cell());
    surface.paint(&|pixmap: &mut PixmapMut<'_>| {
        pixmap.fill(Color::from_rgba8(1, 2, 3, 255));
    });
    surface.clean();
    surface.set_rect(Rect::new(5, 5, 2, 1));
    assert!(!surface.is_dirty(), "a move is a re-place, not a repaint");
    assert_eq!(&surface.pixels()[..4], &[1, 2, 3, 255]);
    surface.set_rect(Rect::new(5, 5, 3, 1));
    assert!(surface.is_dirty());
    assert_eq!(surface.width(), 12);
    assert_eq!(&surface.pixels()[..4], &[0, 0, 0, 0]);
    surface.clean();
    surface.set_cell_size(CellSize::new(8, 8));
    assert!(surface.is_dirty());
    assert_eq!((surface.width(), surface.height()), (24, 8));
}

#[test]
fn an_empty_rect_still_owns_one_pixel() {
    let surface = PixelSurface::new(Rect::new(0, 0, 0, 3), cell());
    assert_eq!((surface.width(), surface.height()), (1, 1));
    assert!(surface.rect().is_empty());
}
