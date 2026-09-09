use super::*;

#[test]
fn the_cell_basis_is_the_identity() {
    let basis = Basis::CELLS;
    assert!(basis.is_cells());
    for cells in [0u16, 1, 7, 40, 120] {
        assert_eq!(basis.across(cells), f32::from(cells));
        assert_eq!(basis.down(cells), f32::from(cells));
    }
}

#[test]
fn a_cell_can_be_a_different_number_of_units_on_each_axis() {
    // The window's own grid: 9 px across, 20 px down. One scalar cannot
    // serve both, which is the whole reason the basis has two.
    let basis = Basis::new(9.0, 20.0);
    assert!(!basis.is_cells());
    assert_eq!(basis.across(3), 27.0);
    assert_eq!(basis.down(3), 60.0);
}

#[test]
fn a_degenerate_axis_is_floored_rather_than_dividing_by_zero() {
    for bad in [0.0, -4.0, f32::NAN, f32::INFINITY] {
        let basis = Basis::new(bad, bad);
        assert_eq!(basis.x(), 1.0, "x for {bad}");
        assert_eq!(basis.y(), 1.0, "y for {bad}");
    }
}

#[test]
fn the_default_basis_is_the_terminals() {
    assert_eq!(Basis::default(), Basis::CELLS);
}
