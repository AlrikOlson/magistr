use super::*;

#[test]
fn an_anchor_names_a_line_and_a_row_into_its_wrap() {
    let anchor = Anchor::new(12, 3);
    assert_eq!(anchor.line(), 12);
    assert_eq!(anchor.row(), 3);
    assert_eq!(anchor, Anchor::new(12, 3));
    assert_ne!(anchor, Anchor::new(12, 0));
}
