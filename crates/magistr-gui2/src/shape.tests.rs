use super::*;

/// Which of the sprite's circles is solid is the shape's own business,
/// because that is how the volume spells the set's signature.
#[test]
fn a_star_dot_is_filled_and_a_ring_is_not() {
    assert!(
        Shape::Star {
            cx: 8.0,
            cy: 8.0,
            r: 1.2
        }
        .is_filled()
    );
    assert!(
        !Shape::Ring {
            cx: 8.0,
            cy: 8.0,
            r: 1.2
        }
        .is_filled()
    );
    assert!(Shape::Fill("M0 0 L4 0 L2 4 Z").is_filled());
    assert!(!Shape::Stroke("M0 0 L4 4").is_filled());
    assert!(
        !Shape::Segment {
            from: (0.0, 0.0),
            to: (4.0, 4.0)
        }
        .is_filled()
    );
}

/// A rounded rectangle covers exactly the box it names — the corner
/// radius rounds inward and puts no ink outside it.
#[test]
fn a_rounded_rect_stays_inside_the_box_it_names() {
    // ic-report's rect, verbatim.
    let path = Shape::Rect {
        x: 4.0,
        y: 2.5,
        w: 8.0,
        h: 11.0,
        r: 1.0,
    }
    .path()
    .expect("the rect builds");
    let bounds = path.bounds();
    assert!((bounds.left() - 4.0).abs() < 0.01, "{bounds:?}");
    assert!((bounds.top() - 2.5).abs() < 0.01, "{bounds:?}");
    assert!((bounds.right() - 12.0).abs() < 0.01, "{bounds:?}");
    assert!((bounds.bottom() - 13.5).abs() < 0.01, "{bounds:?}");
}

/// A radius past half the shorter side is clamped rather than folding
/// the outline inside out.
#[test]
fn an_oversized_radius_is_clamped_to_a_stadium() {
    let path = Shape::Rect {
        x: 0.0,
        y: 0.0,
        w: 10.0,
        h: 4.0,
        r: 40.0,
    }
    .path()
    .expect("the rect builds");
    let bounds = path.bounds();
    assert!((bounds.right() - 10.0).abs() < 0.01, "{bounds:?}");
    assert!((bounds.bottom() - 4.0).abs() < 0.01, "{bounds:?}");
}

/// A rectangle with no area is nothing, not a panic.
#[test]
fn a_degenerate_rect_is_none() {
    assert!(
        Shape::Rect {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 4.0,
            r: 1.0
        }
        .path()
        .is_none()
    );
}

/// ic-orbit is the sprite's one rotated ellipse: rotating it widens the
/// box a 5.6 × 2.6 oval would otherwise have, in both axes.
#[test]
fn a_rotated_ellipse_turns_about_its_own_centre() {
    let flat = Shape::Ellipse {
        cx: 8.0,
        cy: 8.0,
        rx: 5.6,
        ry: 2.6,
        deg: 0.0,
    }
    .path()
    .expect("the oval builds");
    let turned = Shape::Ellipse {
        cx: 8.0,
        cy: 8.0,
        rx: 5.6,
        ry: 2.6,
        deg: -18.0,
    }
    .path()
    .expect("the oval builds");
    // The *tight* bounds, not the control-point box: rotating a
    // cubic's control arms can widen the loose one even where the
    // curve itself narrows.
    let flat = flat.compute_tight_bounds().expect("the oval has extent");
    let turned = turned.compute_tight_bounds().expect("the oval has extent");
    assert!(turned.width() < flat.width(), "{turned:?} vs {flat:?}");
    assert!(turned.height() > flat.height(), "{turned:?} vs {flat:?}");
    // Still centred where it was: a rotation about the centre moves no
    // centre.
    let centre = |r: tiny_skia::Rect| ((r.left() + r.right()) / 2.0, (r.top() + r.bottom()) / 2.0);
    let (cx, cy) = centre(turned);
    assert!((cx - 8.0).abs() < 0.01 && (cy - 8.0).abs() < 0.01);
}

/// Path data outside the reader's subset loses the whole shape rather
/// than most of it.
#[test]
fn geometry_the_reader_refuses_is_no_shape_at_all() {
    assert!(Shape::Stroke("M0 0 C1 1 2 2 3 3").path().is_none());
}
