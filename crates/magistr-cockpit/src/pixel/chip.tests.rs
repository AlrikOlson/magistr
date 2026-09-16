use super::*;
use crate::layout::Rect;
use crate::pixel::{CellSize, PixelSurface};

fn rgb(surface: &PixelSurface, x: u32, y: u32) -> [u8; 3] {
    let width = u32::from(surface.width());
    let at = usize::try_from((y * width + x) * 4).expect("index");
    let px = &surface.pixels()[at..at + 4];
    [px[0], px[1], px[2]]
}

#[test]
fn the_cap_fills_the_middle_and_the_ground_shows_at_the_rounded_corners_and_edges() {
    let mut surface = PixelSurface::new(Rect::new(0, 0, 7, 1), CellSize::new(10, 20));
    surface.paint(&Chip::new((90, 90, 120), (30, 30, 40)));
    assert_eq!(rgb(&surface, 35, 10), [90, 90, 120], "the cap");
    assert_eq!(rgb(&surface, 0, 0), [30, 30, 40], "the corner is the bar");
    assert_eq!(
        rgb(&surface, 35, 0),
        [30, 30, 40],
        "the top inset is the bar"
    );
    assert_eq!(rgb(&surface, 35, 19), [30, 30, 40], "so is the bottom");
}
