use super::*;

#[test]
fn the_scale_is_ordered_and_dense() {
    let scale = [Space::TIGHT, Space::GAP];
    assert!(scale.windows(2).all(|w| w[0] < w[1]), "the steps order");
    assert_eq!(Space::PAD, 1);
    assert_eq!(Space::GAP, 2, "mission-control dense: no airy gaps");
}
