use qs_gpu::frame::PrimKind;

use super::*;

/// The volume's own box, at the design's density.
const FIELD: PxRect = PxRect {
    x: 0.0,
    y: 0.0,
    width: 230.0,
    height: 150.0,
};

fn graph() -> TraceGraph {
    TraceGraph::new(FIELD, 1.0, Medium::Night)
}

fn stop(index: usize) -> LaneStop {
    LaneStop {
        index,
        ..LaneStop::default()
    }
}

fn drawn(graph: &TraceGraph, stops: &[LaneStop]) -> (DrawList, Vec<(usize, PxRect)>) {
    let mut list = DrawList::default();
    let targets = graph.show(stops, &mut list);
    (list, targets)
}

fn count(list: &DrawList, kind: PrimKind) -> usize {
    list.instances
        .iter()
        .filter(|i| i.kind == kind as u32)
        .count()
}

fn centre(rect: PxRect) -> (f32, f32) {
    (rect.x + rect.width * 0.5, rect.y + rect.height * 0.5)
}

/// No stops: nothing drawn, nothing to click — a lane with no steps is
/// not a lane.
#[test]
fn an_empty_lane_draws_nothing_and_answers_no_target() {
    let (list, targets) = drawn(&graph(), &[]);
    assert!(list.instances.is_empty());
    assert!(targets.is_empty());
}

/// Each stop is a dot at the volume's own lane stops — 130, 90, 46, 16
/// up the lane at x=20 — and each answers a click by its feed index.
/// A fifth stop is not on the lane.
#[test]
fn each_stop_is_a_hittable_dot_at_the_volumes_lane_stops() {
    let stops: Vec<LaneStop> = (0..6).map(|i| stop(10 + i)).collect();
    let (list, targets) = drawn(&graph(), &stops);
    assert_eq!(targets.len(), 4, "the lane holds four stops");
    for (slot, ((index, rect), y)) in targets.iter().zip(LANE_STOPS).enumerate() {
        assert_eq!(*index, 10 + slot, "the target carries the feed index");
        let (cx, cy) = centre(*rect);
        assert!((cx - 20.0).abs() < 1e-3, "x={cx}");
        assert!((cy - y).abs() < 1e-3, "y={cy} for stop at {y}");
        assert!(
            rect.width >= 18.0 && rect.height >= 18.0,
            "a click can land"
        );
    }
    // The lane itself plus four dots, every one a plain rect: primitives
    // only, nothing sampled from a texture.
    assert_eq!(count(&list, PrimKind::Rect), 5);
    assert_eq!(count(&list, PrimKind::Glyph), 0);
    assert_eq!(count(&list, PrimKind::Gradient), 0);
}

/// The open step (still being written) is a dashed ring rather than a
/// dot; the focused step wears the accent dot and its ring; a plain
/// step is a dot alone.
#[test]
fn the_open_stop_is_a_dashed_ring_and_the_focused_one_wears_the_accent_ring() {
    let stops = [
        stop(0),
        LaneStop {
            index: 1,
            focused: true,
            ..LaneStop::default()
        },
        LaneStop {
            index: 2,
            open: true,
            ..LaneStop::default()
        },
    ];
    let (list, targets) = drawn(&graph(), &stops);
    assert_eq!(targets.len(), 3);
    assert_eq!(count(&list, PrimKind::DashedStroke), 1, "one open ring");
    assert_eq!(count(&list, PrimKind::Stroke), 1, "one focus ring");
    // The lane, the plain dot and the focused dot: the open stop has no
    // dot under its ring.
    assert_eq!(count(&list, PrimKind::Rect), 3);
    let ring = list
        .instances
        .iter()
        .find(|i| i.kind == PrimKind::Stroke as u32)
        .expect("the focus ring");
    assert!((ring.rect[2] - 18.0).abs() < 1e-3, "r=9 → 18 across");
    assert!((ring.rect[0] + 9.0 - 20.0).abs() < 1e-3 && (ring.rect[1] + 9.0 - 90.0).abs() < 1e-3);
}

/// A fork off a stop is a dashed run of stamps to a tip 130 right and
/// 30 up, the tip a dot that answers the branch step's index; every
/// stamp lands inside the field.
#[test]
fn a_fork_is_dashed_stamps_to_a_hittable_tip() {
    let stops = [
        stop(0),
        LaneStop {
            index: 1,
            fork: Some(7),
            ..LaneStop::default()
        },
        stop(2),
    ];
    let (list, targets) = drawn(&graph(), &stops);
    let tip = targets
        .iter()
        .find(|(index, _)| *index == 7)
        .map(|(_, rect)| centre(*rect))
        .expect("the tip is hittable");
    assert!(
        (tip.0 - 150.0).abs() < 1e-3 && (tip.1 - 60.0).abs() < 1e-3,
        "{tip:?}"
    );
    // Stamps: more than the dots and lane alone, and a dashed walk
    // leaves gaps — fewer than a solid one would.
    let rects = count(&list, PrimKind::Rect);
    let stamps = rects - 5; // lane + 3 dots + tip
    assert!(stamps > 20, "{stamps} stamps");
    let path_len = 80.0 + 50.0; // roughly the cubic plus the run
    assert!(
        (stamps as f32) < path_len / 1.5,
        "dashed: {stamps} stamps under the solid {}",
        path_len / 1.5
    );
    for i in &list.instances {
        assert!(
            i.rect[0] >= -1.0 && i.rect[1] >= -1.0 && i.rect[0] + i.rect[2] <= 231.0,
            "on the field: {:?}",
            i.rect
        );
    }
}

/// Every number scales with the density factor — a HiDPI graph is the
/// same picture.
#[test]
fn the_scale_multiplies_every_number() {
    let scaled = TraceGraph::new(PxRect::new(10.0, 20.0, 460.0, 300.0), 2.0, Medium::Day);
    let (list, targets) = drawn(&scaled, &[stop(0), stop(1)]);
    let (cx, cy) = centre(targets[1].1);
    assert!((cx - (10.0 + 40.0)).abs() < 1e-3 && (cy - (20.0 + 180.0)).abs() < 1e-3);
    let lane = &list.instances[0];
    assert!(
        (lane.rect[2] - 4.0).abs() < 1e-3,
        "the lane is 2 px wide at 2x"
    );
    assert!((lane.rect[3] - (130.0 - 16.0) * 2.0).abs() < 1e-3);
}

/// The two media are one drawing: the same instance count and geometry,
/// only the lexicon's inks differ.
#[test]
fn night_and_day_are_one_drawing_in_two_lexicons() {
    let stops = [
        stop(0),
        LaneStop {
            index: 1,
            focused: true,
            fork: Some(3),
            ..LaneStop::default()
        },
        LaneStop {
            index: 2,
            open: true,
            ..LaneStop::default()
        },
    ];
    let (night, _) = drawn(&TraceGraph::new(FIELD, 1.0, Medium::Night), &stops);
    let (day, _) = drawn(&TraceGraph::new(FIELD, 1.0, Medium::Day), &stops);
    assert_eq!(night.instances.len(), day.instances.len());
    for (n, d) in night.instances.iter().zip(&day.instances) {
        assert_eq!(n.rect, d.rect);
        assert_eq!(n.kind, d.kind);
    }
    assert!(
        night
            .instances
            .iter()
            .zip(&day.instances)
            .any(|(n, d)| n.color != d.color),
        "the inks are the medium's"
    );
}
