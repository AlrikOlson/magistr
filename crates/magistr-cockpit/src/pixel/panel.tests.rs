use super::*;
use crate::layout::Rect;
use crate::pixel::PixelSurface;

fn render(panel: &Panel, cols: u16, rows: u16) -> PixelSurface {
    let mut surface = PixelSurface::with_alpha(Rect::new(0, 0, cols, rows), CellSize::new(10, 20));
    surface.paint(panel);
    surface
}

fn alpha(surface: &PixelSurface, x: u32, y: u32) -> u8 {
    let width = u32::from(surface.width());
    let at = usize::try_from((y * width + x) * 4).expect("index");
    surface.pixels()[at + 3]
}

#[test]
fn the_ring_runs_through_the_middle_of_the_ring_cells_and_the_inside_is_clear() {
    let surface = render(&Panel::new(200, 200, 200, CellSize::new(10, 20)), 12, 6);
    // The left edge: x = 5 (the middle of a 10px cell), halfway down.
    assert!(alpha(&surface, 5, 60) > 100, "the left edge is drawn");
    assert!(alpha(&surface, 115, 60) > 100, "the right edge is drawn");
    assert!(alpha(&surface, 60, 10) > 100, "the top edge is drawn");
    assert!(alpha(&surface, 60, 110) > 100, "the bottom edge is drawn");
    assert_eq!(alpha(&surface, 60, 60), 0, "the inside stays transparent");
    assert_eq!(alpha(&surface, 0, 0), 0, "the corner is rounded off");
}

#[test]
fn the_top_edge_parts_around_the_title_and_a_label_sits_under_it() {
    let plain = render(&Panel::new(200, 200, 200, CellSize::new(10, 20)), 20, 4);
    let titled = render(
        &Panel::new(200, 200, 200, CellSize::new(10, 20)).title(Some((2, 6))),
        20,
        4,
    );
    // Under the title (cells 2..8) the line is gone and the label pill
    // is there, fainter than the line.
    assert!(alpha(&plain, 50, 10) > 100);
    let label = alpha(&titled, 50, 10);
    assert!(
        label > 40 && label < alpha(&plain, 50, 10),
        "a label, not the line: {label}"
    );
    // Past the title the edge resumes.
    assert!(alpha(&titled, 150, 10) > 100);
}

#[test]
fn a_rect_too_small_for_a_ring_draws_nothing() {
    let surface = render(&Panel::new(200, 200, 200, CellSize::new(10, 20)), 1, 1);
    assert!(surface.pixels().chunks(4).all(|px| px[3] == 0));
}
