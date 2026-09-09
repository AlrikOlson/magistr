use super::*;

#[test]
fn edges_are_exclusive_on_the_far_side() {
    let rect = Rect::new(2, 3, 4, 5);
    assert_eq!(rect.right(), 6);
    assert_eq!(rect.bottom(), 8);
    assert!(rect.contains(2, 3));
    assert!(rect.contains(5, 7));
    assert!(!rect.contains(6, 7));
    assert!(!rect.contains(5, 8));
    assert!(!rect.contains(1, 3));
}

#[test]
fn far_edges_saturate() {
    let rect = Rect::new(u16::MAX, u16::MAX, 5, 5);
    assert_eq!(rect.right(), u16::MAX);
    assert_eq!(rect.bottom(), u16::MAX);
}

#[test]
fn an_empty_rect_contains_nothing() {
    assert!(Rect::new(1, 1, 0, 3).is_empty());
    assert!(!Rect::new(1, 1, 0, 3).contains(1, 1));
    assert!(!Rect::new(0, 0, 4, 4).is_empty());
}

#[test]
fn intersection_is_the_overlap() {
    let a = Rect::new(0, 0, 10, 10);
    let b = Rect::new(5, 5, 10, 10);
    assert_eq!(a.intersection(b), Rect::new(5, 5, 5, 5));
    assert_eq!(b.intersection(a), Rect::new(5, 5, 5, 5));
    // fully inside
    assert_eq!(a.intersection(Rect::new(2, 2, 3, 3)), Rect::new(2, 2, 3, 3));
}

#[test]
fn disjoint_rects_intersect_to_empty() {
    let a = Rect::new(0, 0, 5, 5);
    assert_eq!(a.intersection(Rect::new(5, 0, 5, 5)), Rect::default());
    assert_eq!(a.intersection(Rect::new(0, 5, 5, 5)), Rect::default());
    assert!(a.intersection(Rect::new(9, 9, 1, 1)).is_empty());
}
