use super::*;

#[test]
fn the_direct_answer_wins() {
    let cell = CellSize::measured(Some((16, 33)), Some((1716, 2076)), 107, 62);
    assert_eq!(cell, Some(CellSize::new(16, 33)));
}

#[test]
fn the_window_answer_is_divided_by_the_size_in_cells() {
    let cell = CellSize::measured(None, Some((1716, 2076)), 107, 62);
    assert_eq!(cell, Some(CellSize::new(16, 33)));
}

#[test]
fn a_zero_size_in_cells_cannot_divide_and_a_zero_answer_is_no_answer() {
    assert_eq!(CellSize::measured(None, Some((800, 480)), 0, 24), None);
    assert_eq!(CellSize::measured(None, Some((800, 480)), 80, 0), None);
    assert_eq!(CellSize::measured(Some((0, 20)), None, 80, 24), None);
    assert_eq!(CellSize::measured(None, Some((79, 480)), 80, 24), None);
}

#[test]
fn nothing_answered_resolves_to_the_documented_default() {
    assert_eq!(CellSize::measured(None, None, 80, 24), None);
    assert_eq!(CellSize::resolve(None, None, 80, 24), CellSize::DEFAULT);
    assert_eq!(CellSize::DEFAULT, CellSize::new(10, 20));
}

#[test]
fn pixels_scale_a_rect_and_saturate_at_the_wire_limit() {
    assert_eq!(CellSize::new(16, 33).pixels(40, 12), (640, 396));
    assert_eq!(CellSize::new(16, 33).pixels(u16::MAX, 1), (u16::MAX, 33));
}
