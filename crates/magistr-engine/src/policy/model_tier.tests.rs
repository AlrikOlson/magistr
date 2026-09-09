use super::*;

use std::str::FromStr;

#[test]
fn tiers_order_cheapest_first() {
    assert!(ModelTier::Economy < ModelTier::Standard);
    assert!(ModelTier::Standard < ModelTier::Frontier);
}

#[test]
fn round_trips_through_its_string_form() {
    for tier in ModelTier::ALL {
        assert_eq!(ModelTier::from_str(tier.as_str()), Ok(tier));
    }
    assert_eq!(ModelTier::from_str("premium"), Ok(ModelTier::Frontier));
    assert!(ModelTier::from_str("gigantic").is_err());
}
