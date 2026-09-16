use super::*;
use crate::layout::Rect;
use crate::pixel::{CellSize, PixelSurface};

#[test]
fn the_rule_runs_through_the_middle_row_and_leaves_the_rest_clear() {
    let mut surface = PixelSurface::with_alpha(Rect::new(0, 0, 6, 1), CellSize::new(10, 20));
    surface.paint(&Rule::new(200, 100, 50));
    let alpha = |x: u32, y: u32| {
        let at = usize::try_from((y * 60 + x) * 4).expect("index");
        surface.pixels()[at + 3]
    };
    assert!(alpha(30, 10) > 100, "the middle row is drawn");
    assert_eq!(alpha(30, 2), 0, "the top is clear");
    assert_eq!(alpha(30, 17), 0, "the bottom is clear");
    // The pixels are premultiplied: the ink shows through its alpha.
    let at = (10 * 60 + 30) * 4;
    let px = &surface.pixels()[at..at + 4];
    let straight = |c: u8| u32::from(c) * 255 / u32::from(px[3]);
    assert!(
        straight(px[0]).abs_diff(200) <= 2
            && straight(px[1]).abs_diff(100) <= 2
            && straight(px[2]).abs_diff(50) <= 2,
        "the ink: {px:?}"
    );
}
