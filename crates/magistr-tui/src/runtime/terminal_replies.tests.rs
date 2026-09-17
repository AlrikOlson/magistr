use super::*;

#[test]
fn the_direct_answer_wins() {
    let cell = measured(Some((16, 33)), Some((1716, 2076)), 107, 62);
    assert_eq!(cell, Some(CellSize::new(16, 33)));
}

#[test]
fn the_window_answer_is_divided_by_the_size_in_cells() {
    let cell = measured(None, Some((1716, 2076)), 107, 62);
    assert_eq!(cell, Some(CellSize::new(16, 33)));
}

#[test]
fn a_zero_size_in_cells_cannot_divide_and_a_zero_answer_is_no_answer() {
    assert_eq!(measured(None, Some((800, 480)), 0, 24), None);
    assert_eq!(measured(None, Some((800, 480)), 80, 0), None);
    assert_eq!(measured(Some((0, 20)), None, 80, 24), None);
    assert_eq!(measured(None, Some((79, 480)), 80, 24), None);
}

#[test]
fn nothing_answered_resolves_to_the_documented_default() {
    assert_eq!(measured(None, None, 80, 24), None);
    assert_eq!(
        measured(None, None, 80, 24).unwrap_or(CellSize::DEFAULT),
        CellSize::DEFAULT
    );
    assert_eq!(CellSize::DEFAULT, CellSize::new(10, 20));
}
