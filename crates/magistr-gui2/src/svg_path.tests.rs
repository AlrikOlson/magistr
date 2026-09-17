use super::*;

use tiny_skia::PathSegment;

/// Every point a path visits, in order — the verb stream flattened to
/// the coordinates a reader can check by eye against the sprite.
fn points(d: &str) -> Vec<(f32, f32)> {
    let path = svg_path(d).expect("the subset parses");
    let mut out = Vec::new();
    for segment in path.segments() {
        match segment {
            PathSegment::MoveTo(p) | PathSegment::LineTo(p) => out.push((p.x, p.y)),
            PathSegment::QuadTo(a, b) => out.extend([(a.x, a.y), (b.x, b.y)]),
            PathSegment::CubicTo(a, b, c) => {
                out.extend([(a.x, a.y), (b.x, b.y), (c.x, c.y)]);
            }
            PathSegment::Close => {}
        }
    }
    out
}

fn near(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01
}

/// The absolute verbs, which is most of the sprite.
#[test]
fn the_absolute_verbs_land_where_they_say() {
    // ic-check, verbatim.
    assert_eq!(
        points("M3.5 8.6 L6.8 11.6 L12 4.8"),
        vec![(3.5, 8.6), (6.8, 11.6), (12.0, 4.8)]
    );
    // ic-commit: two subpaths, `V` on each.
    assert_eq!(
        points("M8 10.2 V12 M8 4 V5.8"),
        vec![(8.0, 10.2), (8.0, 12.0), (8.0, 4.0), (8.0, 5.8)]
    );
    // ic-settings: `V` and `H` mixed across four subpaths.
    assert_eq!(
        points("M8 3.2 V1.9 M3.2 8 H1.9"),
        vec![(8.0, 3.2), (8.0, 1.9), (3.2, 8.0), (1.9, 8.0)]
    );
}

/// A relative verb is measured from the pen, and `Z` puts the pen back
/// at the subpath's start.
#[test]
fn a_relative_verb_is_measured_from_the_pen() {
    // ic-run, verbatim: `l3.5 3` from (4,5) is (7.5,8); `h2.6` from
    // (9.5,11.5) is (12.1,11.5).
    assert_eq!(
        points("M4 5 l3.5 3 L4 11 M9.5 11.5 h2.6"),
        vec![
            (4.0, 5.0),
            (7.5, 8.0),
            (4.0, 11.0),
            (9.5, 11.5),
            (12.1, 11.5)
        ]
    );
}

/// The pin's `l-.7 3.6 2.2 2.4` is one `l` carrying two points, and the
/// second has no separator before its sign. Both are the real grammar
/// and both appear in the blessed sprite.
#[test]
fn one_verb_may_carry_repeated_parameter_sets() {
    let drawn = points("M6.2 2.8 h3.6 l-.7 3.6 2.2 2.4 H4.7 L6.9 6.4 Z");
    let want = [
        (6.2, 2.8),
        (9.8, 2.8),
        (9.1, 6.4),
        (11.3, 8.8),
        (4.7, 8.8),
        (6.9, 6.4),
    ];
    assert_eq!(drawn.len(), want.len(), "{drawn:?}");
    for (got, expected) in drawn.iter().zip(want) {
        assert!(near(*got, expected), "{drawn:?}");
    }
}

/// An implicit repeat of `M` is a `L`, which is the one place the
/// grammar renames the verb it continues.
#[test]
fn a_repeated_move_is_a_line() {
    assert_eq!(
        points("M1 1 5 5"),
        vec![(1.0, 1.0), (5.0, 5.0)],
        "the second pair continues the move as a line, not a second subpath"
    );
}

/// ic-gate's semicircle: the arc is emitted as cubics that start where
/// the pen was and end exactly where the command says.
#[test]
fn an_arc_starts_and_ends_where_the_command_says() {
    let drawn = points("M3 11 A5 5 0 0 1 13 11");
    assert!(near(drawn[0], (3.0, 11.0)), "starts at the pen: {drawn:?}");
    assert!(
        near(drawn[drawn.len() - 1], (13.0, 11.0)),
        "ends at the command's endpoint: {drawn:?}"
    );
    // A sweep of 180° over a radius of 5 lifts the top of the arc to
    // y = 6; the flag says sweep-positive, which in SVG's y-down frame
    // is the *upper* half.
    let top = drawn.iter().fold(f32::MAX, |acc, p| acc.min(p.1));
    assert!((6.0 - top).abs() < 0.6, "the arc bulges upward: {top}");
}

/// The folder's corners are relative arcs, and its `h-9` has no
/// separator between the verb and the sign.
#[test]
fn the_folders_relative_arcs_close_its_outline() {
    let d = "M2.5 5.2 a1 1 0 0 1 1 -1 h3 l1.5 1.5 h4.5 a1 1 0 0 1 1 1 V11 a1 1 0 0 1 -1 1 h-9 a1 1 0 0 1 -1 -1 Z";
    let path = svg_path(d).expect("the folder parses");
    let bounds = path.bounds();
    assert!(
        bounds.left() >= 2.4 && bounds.right() <= 13.6,
        "inside the 16-unit grid: {bounds:?}"
    );
    assert!(
        bounds.top() >= 4.1 && bounds.bottom() <= 12.1,
        "inside the 16-unit grid: {bounds:?}"
    );
}

/// F.6.6: an ellipse too small to reach both endpoints is scaled up
/// until it just does, rather than refused.
#[test]
fn an_undersized_radius_is_scaled_up_rather_than_refused() {
    let drawn = points("M0 0 A1 1 0 0 1 10 0");
    assert!(near(drawn[drawn.len() - 1], (10.0, 0.0)));
    let top = drawn.iter().fold(f32::MAX, |acc, p| acc.min(p.1));
    assert!((-5.0 - top).abs() < 0.6, "a half circle of radius 5: {top}");
}

/// A degenerate arc is a line, by the specification.
#[test]
fn a_zero_radius_arc_is_a_line() {
    assert_eq!(points("M0 0 A0 0 0 0 1 4 3"), vec![(0.0, 0.0), (4.0, 3.0)]);
}

/// A verb outside the sprite's subset is refused rather than skipped: a
/// glyph missing a stroke has to fail loudly, not draw nearly right.
#[test]
fn a_verb_outside_the_subset_is_refused() {
    assert!(svg_path("M0 0 C1 1 2 2 3 3").is_none());
    assert!(svg_path("M0 0 Q1 1 2 2").is_none());
    assert!(svg_path("M0 0 T2 2").is_none());
}

/// Nothing to draw is nothing, not a panic.
#[test]
fn an_empty_path_is_none() {
    assert!(svg_path("").is_none());
    assert!(svg_path("   ").is_none());
}
