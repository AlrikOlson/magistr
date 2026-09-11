use super::*;

#[test]
fn estimator_matches_javascript_utf16_instead_of_utf8_bytes() {
    assert_eq!(tokens(&"😀😀").expect("wire estimate"), 2);
    assert_eq!(tokens(&"ééé").expect("wire estimate"), 2);
}
