use super::*;
use crate::cost::types::TokenBreakdown;
use chrono::Utc;

fn entry(actor: &str, cost: f64, turns: u32) -> CostEntry {
    CostEntry {
        session_id: format!("s-{actor}-{cost}"),
        project: "magistr".into(),
        timestamp: Utc::now(),
        cost_usd: cost,
        tokens: TokenBreakdown {
            input_tokens: 100,
            output_tokens: 50,
            ..Default::default()
        },
        turns,
        duration_ms: 1000,
        actor: Some(actor.into()),
        model: None,
    }
}

#[test]
fn an_empty_ledger_groups_into_nothing() {
    assert!(summarize_by_dimension(&[], CostDimension::Actor).is_empty());
}

#[test]
fn spend_is_summed_per_key() {
    let entries = vec![
        entry("ada", 1.0, 2),
        entry("ada", 3.0, 1),
        entry("bob", 2.0, 4),
    ];
    let groups = summarize_by_dimension(&entries, CostDimension::Actor);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].key, "ada");
    assert_eq!(groups[0].summary.total_cost, 4.0);
    assert_eq!(groups[0].summary.total_turns, 3);
    assert_eq!(groups[0].summary.entry_count, 2);
    assert_eq!(groups[0].summary.total_tokens.input_tokens, 200);
}

#[test]
fn groups_come_back_biggest_spender_first() {
    let entries = vec![
        entry("ada", 1.0, 1),
        entry("bob", 9.0, 1),
        entry("cy", 5.0, 1),
    ];
    let groups = summarize_by_dimension(&entries, CostDimension::Actor);
    let keys: Vec<&str> = groups.iter().map(|g| g.key.as_str()).collect();
    assert_eq!(keys, vec!["bob", "cy", "ada"]);
}

#[test]
fn share_is_the_fraction_of_total_spend() {
    let entries = vec![entry("ada", 3.0, 1), entry("bob", 1.0, 1)];
    let groups = summarize_by_dimension(&entries, CostDimension::Actor);
    assert!((groups[0].share - 0.75).abs() < 1e-9);
    assert!((groups[1].share - 0.25).abs() < 1e-9);
}

#[test]
fn a_zero_cost_ledger_shares_out_at_zero_rather_than_dividing_by_zero() {
    let entries = vec![entry("ada", 0.0, 1), entry("bob", 0.0, 1)];
    let groups = summarize_by_dimension(&entries, CostDimension::Actor);
    assert_eq!(groups.len(), 2);
    assert!(groups.iter().all(|g| g.share == 0.0));
}

#[test]
fn equal_spenders_are_ordered_by_key_so_the_report_is_stable() {
    let entries = vec![entry("bob", 1.0, 1), entry("ada", 1.0, 1)];
    let groups = summarize_by_dimension(&entries, CostDimension::Actor);
    let keys: Vec<&str> = groups.iter().map(|g| g.key.as_str()).collect();
    assert_eq!(keys, vec!["ada", "bob"]);
}
