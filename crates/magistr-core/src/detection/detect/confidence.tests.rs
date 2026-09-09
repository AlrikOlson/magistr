use super::*;

#[test]
fn confidence_clamps_to_100() {
    let c = Confidence::new(150);
    assert_eq!(c.score(), 100);
}

#[test]
fn confidence_none_is_zero() {
    assert_eq!(Confidence::none().score(), 0);
}

#[test]
fn confidence_levels() {
    assert!(Confidence::high().is_high());
    assert!(!Confidence::high().is_low());
    assert!(!Confidence::medium().is_high());
    assert!(!Confidence::medium().is_low());
    assert!(Confidence::low().is_low());
    assert!(!Confidence::low().is_high());
}

#[test]
fn confidence_labels() {
    assert_eq!(Confidence::new(95).label(), "high");
    assert_eq!(Confidence::new(70).label(), "medium");
    assert_eq!(Confidence::new(40).label(), "low");
    assert_eq!(Confidence::new(10).label(), "none");
}

#[test]
fn confidence_ordering() {
    assert!(Confidence::high() > Confidence::medium());
    assert!(Confidence::medium() > Confidence::low());
    assert!(Confidence::low() > Confidence::none());
}

#[test]
fn confidence_display() {
    assert_eq!(format!("{}", Confidence::new(95)), "95%");
}
