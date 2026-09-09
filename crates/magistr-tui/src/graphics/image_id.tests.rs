use super::*;

#[test]
fn zero_is_not_an_id() {
    assert_eq!(ImageId::new(0), None);
    assert_eq!(ImageId::new(31).map(ImageId::raw), Some(31));
    assert_eq!(ImageId::new(u32::MAX).map(ImageId::raw), Some(u32::MAX));
}

#[test]
fn ids_order_by_raw_value() {
    let a = ImageId::new(1).expect("1");
    let b = ImageId::new(2).expect("2");
    assert!(a < b);
    assert_ne!(a, b);
}
