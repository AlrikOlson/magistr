use super::*;

#[test]
fn round_trips_the_raw_value() {
    assert_eq!(WidgetId::new(7).raw(), 7);
    assert_eq!(WidgetId::new(u32::MAX).raw(), u32::MAX);
}

#[test]
fn equality_and_ordering_follow_the_raw_value() {
    assert_eq!(WidgetId::new(3), WidgetId::new(3));
    assert_ne!(WidgetId::new(3), WidgetId::new(4));
    assert!(WidgetId::new(3) < WidgetId::new(4));
}
