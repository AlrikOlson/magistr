use super::*;
use crate::layout::Rect;
use crate::pixel::{CellSize, PixelSurface};

fn rgb(surface: &PixelSurface, x: u32, y: u32) -> [u8; 3] {
    let width = u32::from(surface.width());
    let at = usize::try_from((y * width + x) * 4).expect("index");
    let px = &surface.pixels()[at..at + 4];
    [px[0], px[1], px[2]]
}

fn render(segment: &Segment) -> PixelSurface {
    let mut surface = PixelSurface::new(Rect::new(0, 0, 8, 1), CellSize::new(10, 20));
    surface.paint(segment);
    surface
}

const TRACK: (u8, u8, u8) = (70, 70, 90);
const GROUND: (u8, u8, u8) = (40, 40, 50);
const FILL: (u8, u8, u8) = (20, 20, 30);

#[test]
fn a_middle_segment_is_track_from_edge_to_edge_and_the_ends_round_off() {
    let middle = render(&Segment::new(TRACK, GROUND));
    assert_eq!(
        rgb(&middle, 0, 10),
        [70, 70, 90],
        "the track reaches the left edge"
    );
    assert_eq!(rgb(&middle, 79, 10), [70, 70, 90], "and the right edge");
    assert_eq!(
        rgb(&middle, 40, 0),
        [40, 40, 50],
        "the inset above is the strip"
    );
    let first = render(&Segment::new(TRACK, GROUND).ends(true, false));
    assert_eq!(
        rgb(&first, 0, 3),
        [40, 40, 50],
        "the first segment's corner is rounded"
    );
    assert_eq!(rgb(&first, 79, 3), [70, 70, 90], "its right side continues");
    let last = render(&Segment::new(TRACK, GROUND).ends(false, true));
    assert_eq!(
        rgb(&last, 79, 3),
        [40, 40, 50],
        "the last segment's corner is rounded"
    );
}

#[test]
fn the_current_segment_carries_a_pill_inside_the_track() {
    let current = render(&Segment::new(TRACK, GROUND).fill(Some(FILL)));
    assert_eq!(rgb(&current, 40, 10), [20, 20, 30], "the pill");
    assert_eq!(
        rgb(&current, 40, 3),
        [70, 70, 90],
        "the track shows above it"
    );
    let other = render(&Segment::new(TRACK, GROUND).fill(None));
    assert_eq!(rgb(&other, 40, 10), [70, 70, 90], "no pill on the others");
}
