use super::*;

use core::str::FromStr;

#[test]
fn tiers_order_least_dangerous_first() {
    assert!(RiskTier::Low < RiskTier::Medium);
    assert!(RiskTier::Medium < RiskTier::High);
}

#[test]
fn round_trips_through_its_string_form() {
    for tier in RiskTier::ALL {
        assert_eq!(RiskTier::from_str(tier.as_str()), Ok(tier));
    }
    assert_eq!(RiskTier::from_str(" MED "), Ok(RiskTier::Medium));
    assert!(RiskTier::from_str("catastrophic").is_err());
}
