use super::*;

#[test]
fn auto_is_the_default() {
    assert_eq!(Length::default(), Length::Auto);
    assert!(Length::Auto.dimension(1.0).is_auto());
    assert!(Length::Auto.length_percentage_auto(1.0).is_auto());
}

#[test]
fn cells_resolve_to_a_fixed_length() {
    let dimension: Dimension = Length::Cells(24).dimension(1.0);
    assert_eq!(dimension, length(24.0));
    let constraint: LengthPercentageAuto = Length::Cells(3).length_percentage_auto(1.0);
    assert_eq!(constraint, length(3.0));
}

#[test]
fn percent_is_a_fraction_of_the_parent() {
    let dimension: Dimension = Length::Percent(0.3).dimension(1.0);
    assert_eq!(dimension, percent(0.3));
    let constraint: LengthPercentageAuto = Length::Percent(0.5).length_percentage_auto(1.0);
    assert_eq!(constraint, percent(0.5));
}

#[test]
fn px_is_engine_units_whatever_the_basis_says_a_cell_is() {
    // 13 px on the window's 20 px-per-cell vertical basis is 13 engine
    // units, not 13 cells — the unit never multiplies in.
    let dimension: Dimension = Length::Px(13.0).dimension(20.0);
    assert_eq!(dimension, length(13.0));
    // And on the cell basis a unit *is* one cell, so Px(1.0) resolves
    // to exactly what Cells(1) resolves to — which is why a stray Px on
    // a terminal tree could never change a cell's arithmetic.
    let px: Dimension = Length::Px(1.0).dimension(1.0);
    let cell: Dimension = Length::Cells(1).dimension(1.0);
    assert_eq!(px, cell);
}
