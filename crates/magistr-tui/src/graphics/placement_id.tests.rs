use super::*;

#[test]
fn zero_is_not_a_placement() {
    assert_eq!(PlacementId::new(0), None);
    assert_eq!(PlacementId::new(2).map(PlacementId::raw), Some(2));
}
