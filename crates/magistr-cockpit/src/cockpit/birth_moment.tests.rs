use super::*;

/// The five moments walk F1–F5 in order, each with its own view id and
/// title, and the flow opens on the ground.
#[test]
fn the_moments_are_the_volumes_five_views_in_order() {
    let views: Vec<&str> = BirthMoment::ALL.iter().map(|m| m.view()).collect();
    assert_eq!(views, ["F1", "F2", "F3", "F4", "F5"]);
    assert_eq!(BirthMoment::default(), BirthMoment::Ground);
    let labels: std::collections::HashSet<&str> =
        BirthMoment::ALL.iter().map(|m| m.label()).collect();
    assert_eq!(labels.len(), 5, "every moment has its own title");
}
