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
fn the_route_mark_is_drawn_on_transparency_in_its_colour() {
    let surface = render(Brand::new(180, 190, 254));
    assert_eq!(surface.format(), PixelFormat::Rgba, "the alpha reaches out");
    assert_eq!(alpha_at(&surface, 0, 0), 0, "top-left corner is clear");
    let (w, h) = (u32::from(surface.width()), u32::from(surface.height()));
    assert_eq!(
        alpha_at(&surface, w - 1, h - 1),
        0,
        "bottom-right corner is clear"
    );
    let node_y = ((h as f32) / 2.0 + 0.16 * (w as f32)) as u32;
    let at = usize::try_from((node_y * w + w / 2) * 4).expect("index");
    let px = &surface.pixels()[at..at + 4];
    assert!(px[3] > 200, "the centre node is opaque: {}", px[3]);
    assert!(px[2] > px[0], "and wears the mark's blue: {px:?}");
}

#[test]
fn the_route_has_three_visible_nodes() {
    let surface = render(Brand::new(255, 255, 255));
    let (w, h) = (u32::from(surface.width()), u32::from(surface.height()));
    let side = w as f32 * 0.78;
    let cy = h as f32 / 2.0;
    for (label, x, y) in [
        ("left", -0.42_f32, -0.40_f32),
        ("centre", 0.0, 0.16),
        ("right", 0.42, -0.40),
    ] {
        let px = (x.mul_add(side, side / 2.0).round() as u32).min(w - 1);
        let py = (y.mul_add(side, cy).round() as u32).min(h - 1);
        assert!(alpha_at(&surface, px, py) > 40, "{label} node");
    }
}
