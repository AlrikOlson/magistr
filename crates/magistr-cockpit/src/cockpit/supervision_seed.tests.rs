use super::*;
use crate::cockpit::{RuleMode, Size};

#[test]
fn the_seed_gives_the_rail_its_charter_budget_and_mode() {
    let seed = SupervisionSeed {
        charter: vec![CharterRule::new("Edit", RuleMode::Ask)],
        offers: Vec::new(),
        budget: Budget::capped(1200),
        mode: "Standard mode".into(),
    };
    let model = seed.clone().seed(Model::new(Size::new(80, 24)));
    assert_eq!(model.rail().charter(), seed.charter.as_slice());
    assert_eq!(model.rail().budget(), seed.budget.as_ref());
    assert_eq!(model.rail().mode(), Some("Standard mode"));
    assert!(model.rail().offers().is_empty());
}
