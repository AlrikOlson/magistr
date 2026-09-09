use super::*;

#[test]
fn agent_budget_unlimited() {
    let budget = AgentBudget::unlimited();
    assert!(!budget.is_exceeded(1_000_000, 100.0, 1000));
    assert!(budget.exceeded_reason(1_000_000, 100.0, 1000).is_none());
}

#[test]
fn agent_budget_token_limit() {
    let budget = AgentBudget {
        max_tokens: 10_000,
        max_cost_usd: 0.0,
        max_turns: 0,
    };
    assert!(!budget.is_exceeded(9_999, 0.0, 0));
    assert!(budget.is_exceeded(10_001, 0.0, 0));

    let reason = budget.exceeded_reason(10_001, 0.0, 0).unwrap();
    assert!(reason.contains("token limit"));
}

#[test]
fn agent_budget_cost_limit() {
    let budget = AgentBudget {
        max_tokens: 0,
        max_cost_usd: 1.0,
        max_turns: 0,
    };
    assert!(!budget.is_exceeded(0, 0.99, 0));
    assert!(budget.is_exceeded(0, 1.01, 0));

    let reason = budget.exceeded_reason(0, 1.01, 0).unwrap();
    assert!(reason.contains("cost limit"));
}

#[test]
fn agent_budget_turn_limit() {
    let budget = AgentBudget {
        max_tokens: 0,
        max_cost_usd: 0.0,
        max_turns: 5,
    };
    assert!(!budget.is_exceeded(0, 0.0, 5));
    assert!(budget.is_exceeded(0, 0.0, 6));

    let reason = budget.exceeded_reason(0, 0.0, 6).unwrap();
    assert!(reason.contains("turn limit"));
}

#[test]
fn pool_budget_unlimited() {
    let budget = PoolBudget::unlimited();
    assert!(!budget.is_exceeded(100.0, 1_000_000));
    assert!(budget.exceeded_reason(100.0, 1_000_000).is_none());
    assert!(budget.can_add_worker(100));
}

#[test]
fn pool_budget_cost_limit() {
    let budget = PoolBudget {
        max_total_cost_usd: 5.0,
        ..PoolBudget::unlimited()
    };
    assert!(!budget.is_exceeded(4.99, 0));
    assert!(budget.is_exceeded(5.01, 0));
}

#[test]
fn pool_budget_token_limit() {
    let budget = PoolBudget {
        max_total_tokens: 50_000,
        ..PoolBudget::unlimited()
    };
    assert!(!budget.is_exceeded(0.0, 49_999));
    assert!(budget.is_exceeded(0.0, 50_001));
}

#[test]
fn pool_budget_concurrency_limit() {
    let budget = PoolBudget {
        max_concurrent_workers: 3,
        ..PoolBudget::unlimited()
    };
    assert!(budget.can_add_worker(0));
    assert!(budget.can_add_worker(2));
    assert!(!budget.can_add_worker(3));
    assert!(!budget.can_add_worker(4));
}

#[test]
fn pool_budget_exceeded_reason() {
    let budget = PoolBudget {
        max_total_cost_usd: 5.0,
        max_total_tokens: 50_000,
        ..PoolBudget::unlimited()
    };

    assert!(budget.exceeded_reason(4.0, 40_000).is_none());

    let reason = budget.exceeded_reason(4.0, 50_001).unwrap();
    assert!(reason.contains("pool token limit"));

    let reason = budget.exceeded_reason(5.01, 0).unwrap();
    assert!(reason.contains("pool cost limit"));
}

// ─── Role weights ─────────────────────────────────────────────────────────

#[test]
fn role_weight_known_roles() {
    assert!((role_weight("coder") - 1.0).abs() < f64::EPSILON);
    assert!((role_weight("researcher") - 0.4).abs() < f64::EPSILON);
    assert!((role_weight("reviewer") - 0.3).abs() < f64::EPSILON);
    assert!((role_weight("tester") - 0.6).abs() < f64::EPSILON);
    assert!((role_weight("documenter") - 0.3).abs() < f64::EPSILON);
}

#[test]
fn role_weight_unknown_role() {
    assert!((role_weight("custom") - 0.5).abs() < f64::EPSILON);
}

// ─── Budget distribution ──────────────────────────────────────────────────

#[test]
fn distribute_budget_empty_roles() {
    let budgets = distribute_budget(10.0, 100_000, &[]);
    assert!(budgets.is_empty());
}

#[test]
fn distribute_budget_unlimited() {
    let budgets = distribute_budget(0.0, 0, &["coder", "reviewer"]);
    assert_eq!(budgets.len(), 2);
    assert!(!budgets[0].is_exceeded(u64::MAX, f64::MAX, u32::MAX));
}

#[test]
fn distribute_budget_by_weight() {
    let roles = &["coder", "researcher"];
    // coder=1.0, researcher=0.4. Total weight=1.4
    let budgets = distribute_budget(14.0, 140_000, roles);
    assert_eq!(budgets.len(), 2);

    // Coder gets 1.0/1.4 = ~71.4% → $10.0, 100_000 tokens
    assert!((budgets[0].max_cost_usd - 10.0).abs() < 0.01);
    assert_eq!(budgets[0].max_tokens, 100_000);

    // Researcher gets 0.4/1.4 = ~28.6% → $4.0, 40_000 tokens
    assert!((budgets[1].max_cost_usd - 4.0).abs() < 0.01);
    assert_eq!(budgets[1].max_tokens, 40_000);
}

#[test]
fn distribute_budget_equal_weights() {
    let budgets = distribute_budget(10.0, 0, &["coder", "coder"]);
    assert_eq!(budgets.len(), 2);
    assert!((budgets[0].max_cost_usd - 5.0).abs() < 0.01);
    assert!((budgets[1].max_cost_usd - 5.0).abs() < 0.01);
}

#[test]
fn distribute_budget_cost_only() {
    let budgets = distribute_budget(10.0, 0, &["reviewer"]);
    assert_eq!(budgets.len(), 1);
    assert!((budgets[0].max_cost_usd - 10.0).abs() < 0.01);
    assert_eq!(budgets[0].max_tokens, 0); // unlimited tokens
}
