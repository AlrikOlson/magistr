use super::*;

#[test]
fn every_variant_maps_onto_the_engine() {
    assert_eq!(Align::Start.taffy(), AlignItems::START);
    assert_eq!(Align::Center.taffy(), AlignItems::CENTER);
    assert_eq!(Align::End.taffy(), AlignItems::END);
    assert_eq!(Align::Stretch.taffy(), AlignItems::STRETCH);
}

#[test]
fn the_default_is_css_stretch() {
    assert_eq!(Align::default(), Align::Stretch);
}
