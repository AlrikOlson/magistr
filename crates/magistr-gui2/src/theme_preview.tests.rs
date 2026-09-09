use super::*;

use crate::element::Kind;

fn surfaces(root: &Element) -> usize {
    let mut n = 0;
    root.each(&mut |_, e| {
        if matches!(e.kind, Kind::Surface) {
            n += 1;
        }
    });
    n
}

#[test]
fn the_sky_is_the_night_ground_with_three_stars() {
    let sky = ThemePreview::sky(132.0, 74.0);
    assert_eq!(sky.fill, Some(ThemePreview::NIGHT));
    // The ground, the two edges' dots (29 + 31 steps, inclusive), three
    // stars.
    assert_eq!(surfaces(&sky), 1 + 29 + 31 + 3);
    // The mauve star is the biggest, at (88, 44) r 3.5.
    let big = sky.children.last().expect("three stars");
    assert_eq!(big.fill, Some(ThemePreview::NIGHT_MAUVE));
    assert_eq!(big.width, Some(7.0));
}

#[test]
fn the_chart_rules_its_paper_every_fourteen_pixels_and_rings_the_mauve_mark() {
    let chart = ThemePreview::chart(132.0, 74.0);
    assert_eq!(chart.fill, Some(ThemePreview::PAPER));
    // 9 vertical (14..126) + 5 horizontal (14..70) lines, the edges'
    // 60 dots, 3 marks, 1 ring.
    assert_eq!(surfaces(&chart), 1 + 9 + 5 + 60 + 3 + 1);
    let ring = chart.children.last().expect("the ring");
    assert!(ring.fill.is_none(), "a ring is a stroke, not a disc");
    assert_eq!(ring.stroke.map(|s| s.1), Some(1.0));
}

#[test]
fn the_split_seam_slants_from_upper_right_to_lower_left() {
    let split = ThemePreview::split(132.0, 74.0);
    // 74 strips + 2 dots on the ground.
    assert_eq!(surfaces(&split), 1 + 74 + 2);
    let strip_left = |i: usize| split.children[i].pin[0].expect("pinned");
    assert!(
        strip_left(0) > strip_left(73),
        "105°: the paper reaches further left at the bottom"
    );
    assert_eq!(strip_left(37).round(), 66.0, "through the centre");
    // The top strip stops short of the corner; a middle one reaches
    // the edge.
    let width_of = |i: usize| split.children[i].width.expect("sized");
    assert!(strip_left(0) + width_of(0) < 132.0);
    assert_eq!((strip_left(37) + width_of(37)).round(), 132.0);
}

#[test]
fn the_previews_scale_with_the_box() {
    let small = ThemePreview::sky(96.0, 54.0);
    let dot = small.children.last().expect("the mauve star");
    let (sx, sy) = scale(96.0, 54.0);
    assert_eq!(dot.pin[0], Some(88.0 * sx - 3.5 * sx));
    assert_eq!(dot.pin[1], Some(44.0 * sy - 3.5 * sx));
}
