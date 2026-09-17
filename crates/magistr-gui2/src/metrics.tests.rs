use super::*;

use crate::text::TextEngine;

fn metrics() -> CellMetrics {
    TextEngine::embedded()
        .metrics(CellMetrics::DEFAULT_PX)
        .expect("the embedded mono face has metrics")
}

/// The grid is whole pixels on both axes, which is what makes a column a
/// column: a fractional width would put each row's left edge at a
/// different subpixel phase.
#[test]
fn every_number_in_the_grid_is_a_whole_pixel() {
    let cell = metrics();
    for value in [cell.width(), cell.height(), cell.baseline()] {
        assert_eq!(value, value.floor(), "{value} is not integral");
        assert!(value >= 1.0, "{value} is not a usable size");
    }
    assert!(
        cell.baseline() <= cell.height(),
        "the baseline is inside the cell"
    );
}

/// The cell is taller than it is wide, the way a terminal cell is —
/// JetBrains Mono advances 0.6 em and its line box is over 1.2 em, so a
/// grid derived from the face's own numbers comes out roughly 1:2. This
/// is the measurement, not an assumption: `Typeface`'s hand-picked
/// 10x20 was a *choice*, and this asserts the derived grid lands in the
/// same neighbourhood rather than that it equals it.
#[test]
fn the_derived_grid_has_a_terminal_cell_s_aspect() {
    let cell = metrics();
    let aspect = cell.height() / cell.width();
    assert!(
        (1.5..=2.5).contains(&aspect),
        "aspect {aspect} — the grid does not look like a text cell"
    );
}

#[test]
fn a_cell_rect_is_its_columns_and_rows_multiplied_out() {
    let cell = metrics();
    let rect = cell.rect(Rect::new(3, 2, 10, 4));
    assert_eq!(
        rect,
        [
            3.0 * cell.width(),
            2.0 * cell.height(),
            10.0 * cell.width(),
            4.0 * cell.height(),
        ]
    );
}

/// `cell_at` is the inverse of `x`/`y` — the whole of "pixel → cell",
/// and the reason the native GUI reuses the terminal's hit registry
/// rather than growing a second one.
#[test]
fn a_pixel_maps_back_to_the_cell_it_was_drawn_in() {
    let cell = metrics();
    let size = Size::new(120, 40);
    for (col, row) in [(0, 0), (1, 1), (47, 13), (119, 39)] {
        // The top-left pixel of the cell, and the last pixel inside it.
        for (dx, dy) in [
            (0.0, 0.0),
            (
                f64::from(cell.width()) - 0.5,
                f64::from(cell.height()) - 0.5,
            ),
        ] {
            let x = f64::from(cell.x(col)) + dx;
            let y = f64::from(cell.y(row)) + dy;
            assert_eq!(cell.cell_at(x, y, size), (col, row), "({x}, {y})");
        }
    }
}

/// A pointer outside the window — negative, or past the last column —
/// lands on a real cell rather than on one the registry has never heard
/// of. winit reports both: a drag can leave the surface, and a resize
/// can shrink the grid under a pointer that has not moved.
#[test]
fn a_pointer_outside_the_grid_is_clamped_into_it() {
    let cell = metrics();
    let size = Size::new(80, 24);
    assert_eq!(cell.cell_at(-40.0, -40.0, size), (0, 0));
    assert_eq!(cell.cell_at(1.0e6, 1.0e6, size), (79, 23));
}

/// A viewport smaller than one cell still yields a layoutable grid: a
/// `Model` with no columns has nothing to lay out, and a window
/// mid-resize legitimately reports one pixel.
#[test]
fn a_viewport_smaller_than_a_cell_still_yields_a_layoutable_grid() {
    let cell = metrics();
    assert_eq!(cell.grid(0, 0), Size::new(1, 1));
    assert_eq!(cell.grid(1, 1), Size::new(1, 1));
    let big = cell.grid(1920, 1080);
    assert_eq!(big.cols, (1920.0 / cell.width()).floor() as u16);
    assert_eq!(big.rows, (1080.0 / cell.height()).floor() as u16);
}

/// The scissor covers the whole rect: a `ceil` on the extent rather than
/// a truncation, so a clip can never cut the last row of pixels off the
/// content it is supposed to be admitting.
#[test]
fn a_scissor_never_cuts_the_rect_it_clips_to() {
    let cell = metrics();
    let rect = Rect::new(2, 3, 7, 5);
    let [x, y, w, h] = cell.scissor(rect);
    let [fx, fy, fw, fh] = cell.rect(rect);
    assert_eq!((x, y), (fx as u32, fy as u32));
    assert!(f64::from(w) >= f64::from(fw), "{w} covers {fw}");
    assert!(f64::from(h) >= f64::from(fh), "{h} covers {fh}");
}
