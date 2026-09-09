use super::*;

#[test]
fn ascii_is_one_column_per_char() {
    assert_eq!(width("abc"), 3);
    assert_eq!(width(""), 0);
}

#[test]
fn cjk_is_two_columns_per_ideograph() {
    assert_eq!(width("中文"), 4);
    assert_eq!(width("日本語テキスト"), 14);
}

#[test]
fn emoji_zwj_sequence_is_one_cluster_two_columns() {
    // family: man, ZWJ, woman, ZWJ, girl, ZWJ, boy — measured per char this
    // would be 8 columns.
    assert_eq!(
        width("\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}"),
        2
    );
    // skin-tone modifier sequence
    assert_eq!(width("\u{1F44D}\u{1F3FD}"), 2);
    // regional indicator pair (flag)
    assert_eq!(width("\u{1F1F8}\u{1F1EA}"), 2);
}

#[test]
fn combining_marks_add_no_columns() {
    assert_eq!(width("e\u{301}"), 1);
    assert_eq!(width("e\u{301}\u{302}"), 1);
    assert_eq!(width("\u{301}"), 0);
}

#[test]
fn control_characters_are_zero_wide() {
    assert_eq!(width("\t"), 0);
    assert_eq!(width("\n"), 0);
    assert_eq!(width("\u{7}"), 0);
    assert_eq!(width("\u{7f}"), 0);
    assert_eq!(width("a\tb\r\nc"), 3);
}

#[test]
fn the_ascii_fast_path_agrees_with_segmentation() {
    let text = "plan \x07execute\tverify\r\n done";
    let segmented: u16 = text.graphemes(true).map(grapheme_width).sum();
    assert_eq!(width(text), segmented);
    assert_eq!(width(text), 23);
}
