use super::*;

#[test]
fn a_box_with_no_area_is_no_path_and_a_real_one_spans_its_corners() {
    assert!(rounded(4.0, 4.0, 4.0, 10.0, 2.0).is_none());
    assert!(rounded(4.0, 4.0, 10.0, 4.0, 2.0).is_none());
    let path = rounded(2.0, 3.0, 12.0, 9.0, 40.0).expect("a path");
    let bounds = path.bounds();
    assert_eq!((bounds.left(), bounds.top()), (2.0, 3.0));
    assert_eq!((bounds.right(), bounds.bottom()), (12.0, 9.0));
}
