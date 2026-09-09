use super::*;
use crate::render::Color;

fn symbols(buffer: &Buffer, row: u16) -> Vec<&str> {
    (0..buffer.width())
        .map(|c| {
            buffer
                .get(c, row)
                .map(|cell| cell.symbol.as_str())
                .unwrap_or("?")
        })
        .collect()
}

#[test]
fn new_buffer_is_blank() {
    let buffer = Buffer::new(3, 2);
    assert_eq!(buffer.cells().len(), 6);
    assert!(buffer.cells().iter().all(|c| c.symbol == " "));
    assert_eq!(buffer.width(), 3);
    assert_eq!(buffer.height(), 2);
}

#[test]
fn put_str_writes_one_cell_per_narrow_grapheme() {
    let mut buffer = Buffer::new(5, 1);
    let used = buffer.put_str(1, 0, "ab", &Style::new().fg(Color::Indexed(4)));
    assert_eq!(used, 2);
    assert_eq!(symbols(&buffer, 0), [" ", "a", "b", " ", " "]);
    assert_eq!(
        buffer.get(1, 0).map(|c| c.style.fg),
        Some(Color::Indexed(4))
    );
}

#[test]
fn wide_grapheme_occupies_head_and_continuation() {
    let mut buffer = Buffer::new(4, 1);
    assert_eq!(buffer.put_str(0, 0, "中x", &Style::new()), 3);
    assert_eq!(symbols(&buffer, 0), ["中", "", "x", " "]);
    assert!(buffer.get(1, 0).is_some_and(Cell::is_continuation));
}

#[test]
fn combining_mark_stays_in_its_base_cell() {
    let mut buffer = Buffer::new(3, 1);
    assert_eq!(buffer.put_str(0, 0, "e\u{301}x", &Style::new()), 2);
    assert_eq!(symbols(&buffer, 0), ["e\u{301}", "x", " "]);
}

#[test]
fn zwj_family_is_one_head_cell_two_columns() {
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}";
    let mut buffer = Buffer::new(3, 1);
    assert_eq!(buffer.put_str(0, 0, family, &Style::new()), 2);
    assert_eq!(symbols(&buffer, 0), [family, "", " "]);
}

#[test]
fn text_is_clipped_at_the_right_edge() {
    let mut buffer = Buffer::new(3, 1);
    assert_eq!(buffer.put_str(1, 0, "abcdef", &Style::new()), 2);
    assert_eq!(symbols(&buffer, 0), [" ", "a", "b"]);
}

#[test]
fn wide_grapheme_at_the_last_column_becomes_a_blank() {
    let mut buffer = Buffer::new(3, 1);
    buffer.fill(&Cell::new("#", Style::new()));
    assert_eq!(buffer.put_str(1, 0, "a中b", &Style::new()), 2);
    assert_eq!(symbols(&buffer, 0), ["#", "a", " "]);
}

#[test]
fn overwriting_a_continuation_blanks_the_head() {
    let mut buffer = Buffer::new(3, 1);
    buffer.put_str(0, 0, "中", &Style::new());
    buffer.set(1, 0, Cell::new("x", Style::new()));
    assert_eq!(symbols(&buffer, 0), [" ", "x", " "]);
}

#[test]
fn overwriting_a_head_blanks_the_continuation() {
    let mut buffer = Buffer::new(3, 1);
    buffer.put_str(1, 0, "中", &Style::new());
    buffer.set(1, 0, Cell::new("x", Style::new()));
    assert_eq!(symbols(&buffer, 0), [" ", "x", " "]);
}

#[test]
fn a_wide_write_over_two_wide_runs_repairs_both() {
    // 中 at 0-1, 文 at 2-3; writing 字 at 1-2 must blank 0 and 3.
    let mut buffer = Buffer::new(4, 1);
    buffer.put_str(0, 0, "中文", &Style::new());
    buffer.set(1, 0, Cell::new("字", Style::new()));
    assert_eq!(symbols(&buffer, 0), [" ", "字", "", " "]);
}

#[test]
fn out_of_range_writes_are_dropped_without_panic() {
    let mut buffer = Buffer::new(2, 2);
    buffer.set(2, 0, Cell::new("x", Style::new()));
    buffer.set(0, 2, Cell::new("x", Style::new()));
    buffer.set(u16::MAX, u16::MAX, Cell::new("x", Style::new()));
    assert_eq!(buffer.put_str(5, 0, "abc", &Style::new()), 0);
    assert_eq!(buffer.put_str(0, 9, "abc", &Style::new()), 0);
    assert!(buffer.get(2, 0).is_none());
    assert!(buffer.get(0, 2).is_none());
    assert!(buffer.cells().iter().all(|c| c.symbol == " "));
}

#[test]
fn zero_width_graphemes_are_skipped() {
    let mut buffer = Buffer::new(4, 1);
    assert_eq!(buffer.put_str(0, 0, "a\tb\u{301}\n", &Style::new()), 2);
    assert_eq!(symbols(&buffer, 0), ["a", "b\u{301}", " ", " "]);
}

#[test]
fn set_with_a_continuation_or_zero_width_cell_stores_a_blank() {
    let mut buffer = Buffer::new(2, 1);
    buffer.set(0, 0, Cell::continuation());
    buffer.set(
        1,
        0,
        Cell::new("\u{301}", Style::new().bg(Color::Indexed(1))),
    );
    assert_eq!(symbols(&buffer, 0), [" ", " "]);
    assert_eq!(
        buffer.get(1, 0).map(|c| c.style.bg),
        Some(Color::Indexed(1))
    );
}

#[test]
fn resize_yields_a_blank_grid_of_the_new_size() {
    let mut buffer = Buffer::new(2, 1);
    buffer.put_str(0, 0, "ab", &Style::new());
    buffer.resize(3, 2);
    assert_eq!((buffer.width(), buffer.height()), (3, 2));
    assert!(buffer.cells().iter().all(|c| c.symbol == " "));
    buffer.resize(0, 0);
    assert!(buffer.cells().is_empty());
    assert!(buffer.get(0, 0).is_none());
}

#[test]
fn fill_with_a_wide_cell_falls_back_to_a_blank() {
    let mut buffer = Buffer::new(2, 1);
    buffer.fill(&Cell::new("中", Style::new().fg(Color::Indexed(2))));
    assert_eq!(symbols(&buffer, 0), [" ", " "]);
    assert_eq!(
        buffer.get(0, 0).map(|c| c.style.fg),
        Some(Color::Indexed(2))
    );
}
