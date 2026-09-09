use super::*;
use crate::render::Color;

#[test]
fn blank_is_a_single_space_in_default_style() {
    let cell = Cell::default();
    assert_eq!(cell.symbol, " ");
    assert_eq!(cell.style, Style::default());
    assert_eq!(cell.width(), 1);
    assert!(!cell.is_continuation());
}

#[test]
fn continuation_has_no_symbol_and_no_width() {
    let cell = Cell::continuation();
    assert!(cell.is_continuation());
    assert_eq!(cell.width(), 0);
}

#[test]
fn wide_grapheme_reports_two_columns() {
    let cell = Cell::new("中", Style::new().fg(Color::Rgb(1, 2, 3)));
    assert_eq!(cell.width(), 2);
    assert_eq!(cell.style.fg, Color::Rgb(1, 2, 3));
}
