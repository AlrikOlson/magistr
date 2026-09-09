use super::*;

#[test]
fn the_readout_is_the_captures_spelling() {
    let budget = Budget::capped(1200).expect("a positive cap").spent(184);
    assert_eq!(budget.readout(), "1.84 / 12.00");
}

#[test]
fn an_unmetered_budget_still_shows_its_cap() {
    let budget = Budget::capped(1200).expect("a positive cap");
    assert_eq!(budget.readout(), "— / 12.00");
    assert_eq!(budget.fill_per_mille(), 0, "an empty meter, not a guess");
    assert_eq!(budget.breaker(), None, "no breaker line without a stop");
}

#[test]
fn the_breaker_and_tick_sit_at_the_stop() {
    let budget = Budget::capped(1200)
        .expect("a positive cap")
        .spent(184)
        .stop_at(1000);
    assert_eq!(budget.breaker().as_deref(), Some("STOP AT 10.00"));
    assert_eq!(budget.tick_per_mille(), Some(833), "10.00 of 12.00");
    assert_eq!(budget.fill_per_mille(), 153, "1.84 of 12.00");
}

#[test]
fn a_zero_cap_is_no_budget_at_all() {
    assert_eq!(Budget::capped(0), None, "a meter to nowhere is furniture");
}

#[test]
fn a_spend_past_the_limit_pins_the_meter_full() {
    let budget = Budget::capped(100).expect("a positive cap").spent(250);
    assert_eq!(budget.fill_per_mille(), 1000);
    assert_eq!(budget.readout(), "2.50 / 1.00");
}
