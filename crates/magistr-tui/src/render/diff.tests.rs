use super::*;
use magistr_cockpit::render::{Cell, Style};

#[test]
fn identical_buffers_have_no_changes() {
    let mut a = Buffer::new(4, 2);
    a.put_str(0, 0, "ab中", &Style::new());
    let b = a.clone();
    assert!(diff(&a, &b).is_empty());
}

#[test]
fn only_changed_cells_are_reported_in_row_major_order() {
    let mut prev = Buffer::new(3, 2);
    prev.put_str(0, 0, "abc", &Style::new());
    let mut next = prev.clone();
    next.set(2, 0, Cell::new("X", Style::new()));
    next.set(0, 1, Cell::new("Y", Style::new()));
    let changes = diff(&prev, &next);
    let positions: Vec<(u16, u16, &str)> = changes
        .iter()
        .map(|c| (c.col, c.row, c.cell.symbol.as_str()))
        .collect();
    assert_eq!(positions, [(2, 0, "X"), (0, 1, "Y")]);
}

#[test]
fn continuation_cells_are_never_reported() {
    let prev = Buffer::new(3, 1);
    let mut next = Buffer::new(3, 1);
    next.put_str(0, 0, "中", &Style::new());
    let changes = diff(&prev, &next);
    assert_eq!(changes.len(), 1);
    assert_eq!((changes[0].col, changes[0].cell.symbol.as_str()), (0, "中"));
}

#[test]
fn replacing_a_wide_glyph_with_narrow_ones_reports_both_columns() {
    let mut prev = Buffer::new(2, 1);
    prev.put_str(0, 0, "中", &Style::new());
    let mut next = Buffer::new(2, 1);
    next.put_str(0, 0, "ab", &Style::new());
    let cols: Vec<u16> = diff(&prev, &next).iter().map(|c| c.col).collect();
    assert_eq!(cols, [0, 1]);
}

#[test]
fn a_style_only_change_is_a_change() {
    let prev = Buffer::new(1, 1);
    let mut next = Buffer::new(1, 1);
    next.set(
        0,
        0,
        Cell::new(
            " ",
            Style::new().bg(magistr_cockpit::render::Color::Indexed(1)),
        ),
    );
    assert_eq!(diff(&prev, &next).len(), 1);
}

#[test]
fn different_sizes_report_every_head_cell() {
    let prev = Buffer::new(2, 1);
    let mut next = Buffer::new(3, 1);
    next.put_str(0, 0, "中", &Style::new());
    let cols: Vec<u16> = diff(&prev, &next).iter().map(|c| c.col).collect();
    assert_eq!(cols, [0, 2]);
    assert!(diff(&prev, &Buffer::new(0, 5)).is_empty());
}
