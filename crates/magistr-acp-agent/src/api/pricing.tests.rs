use super::*;

#[test]
fn cost_is_per_million_on_each_side() {
    let gpt4o = Pricing::per_million(2.50, 10.00);
    let cost = gpt4o.cost(1_000_000, 100_000);
    assert!((cost - 3.50).abs() < 1e-9, "{cost}");
}

#[test]
fn free_costs_nothing_whatever_the_counts() {
    assert_eq!(Pricing::FREE.cost(5_000_000, 5_000_000), 0.0);
}

#[test]
fn zero_tokens_cost_zero() {
    assert_eq!(Pricing::per_million(30.0, 60.0).cost(0, 0), 0.0);
}
