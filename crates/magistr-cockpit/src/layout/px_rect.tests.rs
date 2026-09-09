use super::*;

#[test]
fn on_the_cell_basis_a_px_rect_is_its_own_cell_rect() {
    let px = PxRect::new(3.0, 4.0, 10.0, 2.0);
    assert_eq!(px.to_cells(Basis::CELLS), Rect::new(3, 4, 10, 2));
}

#[test]
fn a_window_basis_divides_each_axis_by_its_own_cell() {
    let basis = Basis::new(9.0, 20.0);
    let px = PxRect::new(27.0, 40.0, 90.0, 60.0);
    assert_eq!(px.to_cells(basis), Rect::new(3, 2, 10, 3));
}

#[test]
fn the_edges_round_so_abutting_rects_stay_abutting() {
    // Two rects meeting at 10.5 units: rounding the *edges* sends both
    // to column 11 (round-half-away-from-zero), so no column is claimed
    // twice and none is left blank. Rounding the widths could not
    // promise that.
    let left = PxRect::new(0.0, 0.0, 10.5, 1.0).to_cells(Basis::CELLS);
    let right = PxRect::new(10.5, 0.0, 9.5, 1.0).to_cells(Basis::CELLS);
    assert_eq!(left.right(), right.x);
    assert_eq!(right.right(), 20);
}

#[test]
fn an_empty_or_inverted_rect_covers_nothing() {
    assert!(PxRect::default().is_empty());
    assert!(PxRect::new(2.0, 2.0, -4.0, 5.0).is_empty());
    assert!(!PxRect::new(0.0, 0.0, 1.0, 1.0).is_empty());
}

#[test]
fn containment_owns_the_left_and_top_edges_only() {
    let rect = PxRect::new(10.0, 20.0, 30.0, 40.0);
    assert!(rect.contains(10.0, 20.0));
    assert!(rect.contains(39.9, 59.9));
    assert!(!rect.contains(40.0, 30.0));
    assert!(!rect.contains(20.0, 60.0));
    assert!(!rect.contains(9.9, 30.0));
}

#[test]
fn disjoint_rects_intersect_to_nothing() {
    let a = PxRect::new(0.0, 0.0, 5.0, 5.0);
    let b = PxRect::new(6.0, 6.0, 5.0, 5.0);
    assert!(a.intersection(b).is_empty());
    assert_eq!(
        a.intersection(PxRect::new(2.0, 1.0, 10.0, 2.0)),
        PxRect::new(2.0, 1.0, 3.0, 2.0)
    );
}
