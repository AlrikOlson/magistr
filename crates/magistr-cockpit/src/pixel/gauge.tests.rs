use super::*;
use crate::layout::Rect;
use crate::pixel::{CellSize, PixelSurface};

fn render(gauge: &Gauge, cols: u16, rows: u16) -> PixelSurface {
    let mut surface = PixelSurface::new(Rect::new(0, 0, cols, rows), CellSize::new(10, 20));
    surface.paint(gauge);
    surface
}

/// The x of the last column whose middle row is brighter than the track
/// — where the fill ends, in pixels.
fn fill_edge(surface: &PixelSurface) -> Option<u32> {
    let width = u32::from(surface.width());
    let y = u32::from(surface.height()) / 2;
    (0..width).rev().find(|x| {
        let at = usize::try_from((y * width + x) * 4).expect("index");
        let px = &surface.pixels()[at..at + 4];
        u16::from(px[0]) + u16::from(px[1]) + u16::from(px[2]) > 350
    })
}

#[test]
fn the_fill_lands_on_the_pixel_not_the_cell() {
    // Two fills a single cell apart in a 10-cell bar would round to the
    // same cell; the gauge separates them by pixels.
    let a = fill_edge(&render(&Gauge::new(0.50), 10, 1)).expect("a fill");
    let b = fill_edge(&render(&Gauge::new(0.53), 10, 1)).expect("a fill");
    assert!(b > a, "0.53 reaches past 0.50: {b} > {a}");
    assert!(b - a < 10, "and by less than one 10px cell: {}", b - a);
}

#[test]
fn a_full_gauge_reaches_the_end_and_an_empty_one_is_track_only() {
    let full = render(&Gauge::new(1.0), 8, 1);
    let edge = fill_edge(&full).expect("a fill");
    assert!(
        edge >= u32::from(full.width()) - 2,
        "the full bar reaches the end: {edge} of {}",
        full.width()
    );
    assert_eq!(
        fill_edge(&render(&Gauge::new(0.0), 8, 1)),
        None,
        "an empty gauge draws the track alone"
    );
}

#[test]
fn the_fraction_is_clamped_and_a_non_finite_one_is_empty() {
    assert_eq!(Gauge::new(2.0).fill(), 1.0);
    assert_eq!(Gauge::new(-1.0).fill(), 0.0);
    assert_eq!(Gauge::new(f32::NAN).fill(), 0.0);
}

#[test]
fn the_track_sits_under_the_fill_in_the_same_hue() {
    let surface = render(&Gauge::new(0.5).line(200, 40, 40).background(0, 0, 0), 8, 1);
    let width = u32::from(surface.width());
    let y = u32::from(surface.height()) / 2;
    let sample = |x: u32| {
        let at = usize::try_from((y * width + x) * 4).expect("index");
        let px = &surface.pixels()[at..at + 4];
        [px[0], px[1], px[2]]
    };
    let [fr, fg, fb] = sample(width / 4);
    let [tr, tg, tb] = sample(width * 7 / 8);
    assert!(fr > tr, "the fill is brighter than the track: {fr} > {tr}");
    assert!(
        tr > tg && tr > tb,
        "the track keeps the hue: {tr},{tg},{tb}"
    );
    assert!(fr > fg && fr > fb, "so does the fill: {fr},{fg},{fb}");
}

#[test]
fn a_bar_narrower_than_its_caps_draws_nothing() {
    // A 1x4-cell surface is 10px wide and 80px tall: the thickness is
    // capped at the height, so the caps eat the whole span.
    let surface = render(&Gauge::new(1.0), 1, 4);
    assert_eq!(fill_edge(&surface), None, "no room, no bar");
}
