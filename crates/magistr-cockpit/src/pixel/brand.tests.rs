use super::*;
use crate::layout::Rect;
use crate::pixel::PixelFormat;
use crate::pixel::{CellSize, PixelSurface};

fn render(brand: Brand) -> PixelSurface {
    let mut surface = PixelSurface::with_alpha(Rect::new(0, 0, 1, 1), CellSize::new(16, 32));
    surface.paint(&brand);
    surface
}

fn alpha_at(surface: &PixelSurface, x: u32, y: u32) -> u8 {
    let width = u32::from(surface.width());
    let at = usize::try_from((y * width + x) * 4).expect("index");
    surface.pixels()[at + 3]
}

#[test]
fn the_mark_is_drawn_on_transparency_in_its_colour() {
    let surface = render(Brand::new(180, 190, 254));
    assert_eq!(surface.format(), PixelFormat::Rgba, "the alpha reaches out");
    // The corners are outside the peak: nothing there.
    assert_eq!(alpha_at(&surface, 0, 0), 0, "top-left corner is clear");
    let (w, h) = (u32::from(surface.width()), u32::from(surface.height()));
    assert_eq!(alpha_at(&surface, w - 1, 0), 0, "top-right corner is clear");
    // The peak's body, above the crossbar slot, is opaque and the
    // mark's colour comes through.
    let body = h / 2 - w / 5;
    let at = usize::try_from((body * w + w / 2) * 4).expect("index");
    let px = &surface.pixels()[at..at + 4];
    assert!(px[3] > 200, "the body is opaque: {}", px[3]);
    assert!(px[2] > px[0], "and wears the mark's blue: {px:?}");
}

#[test]
fn the_crossbar_slot_is_a_hole_through_the_peak() {
    let surface = render(Brand::new(255, 255, 255));
    let (w, h) = (u32::from(surface.width()), u32::from(surface.height()));
    let side = f32::from(surface.width()).min(f32::from(surface.height()));
    let row = |unit: f32| (f32::from(surface.height()) / 2.0).mul_add(1.0, unit * side) as u32;
    let centre = w / 2;
    // Down the mark's centre line: solid above the slot, clear inside
    // it, solid again below.
    assert!(
        alpha_at(&surface, centre, row(-0.10).min(h - 1)) > 200,
        "above"
    );
    assert!(
        alpha_at(&surface, centre, row(0.15).min(h - 1)) < 40,
        "inside"
    );
    assert!(
        alpha_at(&surface, centre, row(0.34).min(h - 1)) > 200,
        "below"
    );
}
