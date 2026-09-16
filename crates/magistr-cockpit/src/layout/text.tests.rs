use super::*;

#[test]
fn intrinsic_widths_come_from_the_widest_line_and_word() {
    let text = TextMetrics::new("plan execute\nverify it");
    assert_eq!(text.max_content_width(), 12);
    assert_eq!(text.min_content_width(), 7);
    assert_eq!(text.text(), "plan execute\nverify it");
}

#[test]
fn empty_text_measures_zero() {
    let text = TextMetrics::new("");
    assert_eq!(text.max_content_width(), 0);
    assert_eq!(text.min_content_width(), 0);
    assert_eq!(text.wrapped_height(10), 0);
    assert!(text.wrap(10).is_empty());
}

#[test]
fn wraps_greedily_by_word() {
    let text = TextMetrics::new("the quick brown fox jumps");
    assert_eq!(text.wrap(10), vec!["the quick", "brown fox", "jumps"]);
    assert_eq!(text.wrapped_height(10), 3);
    assert_eq!(text.wrap(25), vec!["the quick brown fox jumps"]);
    assert_eq!(text.wrapped_height(25), 1);
}

#[test]
fn a_word_exactly_filling_the_row_fits() {
    let text = TextMetrics::new("abcde fghij");
    assert_eq!(text.wrap(5), vec!["abcde", "fghij"]);
}

#[test]
fn newlines_force_rows_and_blank_lines_keep_a_row() {
    let text = TextMetrics::new("one\n\ntwo three");
    assert_eq!(text.wrap(5), vec!["one", "", "two", "three"]);
    assert_eq!(text.wrapped_height(5), 4);
}

#[test]
fn long_words_break_at_grapheme_boundaries() {
    let text = TextMetrics::new("abcdefghij kl");
    assert_eq!(text.wrap(4), vec!["abcd", "efgh", "ij", "kl"]);
}

#[test]
fn a_broken_word_tail_takes_following_words() {
    let text = TextMetrics::new("abcdefg hi");
    assert_eq!(text.wrap(5), vec!["abcde", "fg hi"]);
}

#[test]
fn wide_graphemes_count_two_columns() {
    let text = TextMetrics::new("日本語 テキスト");
    assert_eq!(text.max_content_width(), 15);
    assert_eq!(text.min_content_width(), 8);
    assert_eq!(text.wrap(8), vec!["日本語", "テキスト"]);
    assert_eq!(text.wrap(3), vec!["日", "本", "語", "テ", "キ", "ス", "ト"]);
}

#[test]
fn a_grapheme_wider_than_the_row_still_takes_a_row() {
    let text = TextMetrics::new("中a");
    assert_eq!(text.wrap(1), vec!["中", "a"]);
}

#[test]
fn zero_columns_yield_no_rows() {
    let text = TextMetrics::new("anything");
    assert!(text.wrap(0).is_empty());
    assert_eq!(text.wrapped_height(0), 0);
}

#[test]
fn repeated_spaces_are_measured_as_the_original_text() {
    let text = TextMetrics::new("a  b");
    assert_eq!(text.wrap(10), vec!["a  b"]);
    assert_eq!(text.wrap(3), vec!["a", "b"]);
    assert_eq!(text.wrap(1), vec!["a", "b"]);
    let title = TextMetrics::new("  4  Compare limits (Share retry budgets?)");
    for width in 1..40 {
        assert!(
            title
                .wrap(width)
                .iter()
                .all(|row| crate::render::width(row) <= width || row.trim().is_empty())
        );
    }
}

#[test]
fn leading_spaces_stay_on_the_first_row() {
    let code = TextMetrics::new("    let x = 1;");
    assert_eq!(code.wrap(40), vec!["    let x = 1;"]);
    assert_eq!(code.wrapped_height(40), 1);
    assert_eq!(code.wrap(9), vec!["    let x", "= 1;"]);
    let item = TextMetrics::new("  continuation of a bullet");
    assert_eq!(item.wrap(14), vec!["  continuation", "of a bullet"]);
    let one = TextMetrics::new(" a b");
    assert_eq!(one.wrap(4), vec![" a b"]);
    assert_eq!(one.wrap(3), vec![" a", "b"]);
    let blank = TextMetrics::new("   ");
    assert_eq!(blank.wrap(10), vec!["   "]);
    let deep = TextMetrics::new("        x");
    assert_eq!(
        deep.wrap(4),
        vec!["        ", "x"],
        "an indent wider than the row still shows"
    );
}

#[test]
fn wrapping_by_columns_is_wrapping_by_a_ruler_that_counts_columns() {
    let text = TextMetrics::new("the quick brown fox jumps over the lazy dog");
    for columns in [1u16, 4, 9, 12, 20, 43, 80] {
        assert_eq!(
            text.wrap(columns),
            text.wrap_by(f32::from(columns), &mut |run| f32::from(width(run))),
            "at {columns} columns"
        );
    }
}

#[test]
fn a_wider_ruler_breaks_a_paragraph_in_fewer_places() {
    // A proportional face is narrower than a monospaced one at the same
    // size, which is why a native row fits more of a paragraph than the
    // column count says. The rule is the same greedy one either way.
    let text = TextMetrics::new("alpha beta gamma delta epsilon");
    let half = &mut |run: &str| f32::from(width(run)) * 0.5;
    assert!(text.wrap_by(10.0, half).len() < text.wrap(10).len());
}

#[test]
fn a_ruler_with_no_room_wraps_to_nothing() {
    let text = TextMetrics::new("anything");
    assert!(text.wrap_by(0.0, &mut |_| 1.0).is_empty());
    assert!(text.wrap_by(-3.0, &mut |_| 1.0).is_empty());
}

#[test]
fn a_word_wider_than_the_ruler_still_breaks_at_graphemes() {
    let text = TextMetrics::new("ありがとう");
    // Two units a grapheme, four units a row: two graphemes a row.
    let rows = text.wrap_by(4.0, &mut |run| f32::from(width(run)));
    assert_eq!(rows, ["あり", "がと", "う"]);
}
