use super::*;

#[test]
fn every_variant_maps_onto_the_engine() {
    assert_eq!(Justify::Start.taffy(), JustifyContent::START);
    assert_eq!(Justify::Center.taffy(), JustifyContent::CENTER);
    assert_eq!(Justify::End.taffy(), JustifyContent::END);
    assert_eq!(Justify::SpaceBetween.taffy(), JustifyContent::SPACE_BETWEEN);
    assert_eq!(Justify::SpaceAround.taffy(), JustifyContent::SPACE_AROUND);
    assert_eq!(Justify::SpaceEvenly.taffy(), JustifyContent::SPACE_EVENLY);
}

#[test]
fn the_default_is_css_start() {
    assert_eq!(Justify::default(), Justify::Start);
}
