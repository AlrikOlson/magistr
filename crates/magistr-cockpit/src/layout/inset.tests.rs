use taffy::prelude::{auto, length, percent};

use super::*;

#[test]
fn none_leaves_every_edge_unconstrained() {
    let inset = Inset::none().taffy(Basis::CELLS);
    assert_eq!(inset.left, auto());
    assert_eq!(inset.right, auto());
    assert_eq!(inset.top, auto());
    assert_eq!(inset.bottom, auto());
    assert_eq!(Inset::default(), Inset::none());
}

#[test]
fn all_sets_the_same_distance_on_every_edge() {
    let inset = Inset::all(2).taffy(Basis::CELLS);
    assert_eq!(inset.left, length(2.0));
    assert_eq!(inset.right, length(2.0));
    assert_eq!(inset.top, length(2.0));
    assert_eq!(inset.bottom, length(2.0));
}

#[test]
fn edges_can_be_set_one_at_a_time_and_mix_units() {
    let inset = Inset::none()
        .top(Length::Cells(1))
        .right(Length::Percent(0.25))
        .taffy(Basis::CELLS);
    assert_eq!(inset.top, length(1.0));
    assert_eq!(inset.right, percent(0.25));
    assert_eq!(inset.left, auto(), "an unset edge stays unconstrained");
    assert_eq!(inset.bottom, auto());
}
