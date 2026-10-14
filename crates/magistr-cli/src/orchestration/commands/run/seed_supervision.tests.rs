use magistr_engine::cost::{BudgetPeriod, BudgetRule, BudgetScope};

use super::*;
use crate::team::directory::DirectoryConfig;
use crate::team::{TeamConfig, TeamRole};

fn rows(model: &SupervisionSeed) -> Vec<(String, RuleMode)> {
    model
        .charter
        .iter()
        .map(|rule| (rule.subject().to_owned(), rule.mode()))
        .collect()
}

/// A roster in `dir` with the given budget rules and no governance rule.
fn roster_with(dir: &Path, budgets: Vec<BudgetRule>) {
    TeamConfig {
        version: 1,
        default_role: TeamRole::Developer,
        identity_provider: None,
        budgets,
        policy: Default::default(),
        members: Vec::new(),
        directory: DirectoryConfig::default(),
    }
    .save(dir)
    .expect("save roster");
}

#[test]
fn the_effects_fold_to_the_three_badges() {
    assert_eq!(mode(PolicyEffect::RequireApproval), RuleMode::Ask);
    assert_eq!(mode(PolicyEffect::Allow), RuleMode::Auto);
    assert_eq!(mode(PolicyEffect::Warn), RuleMode::Auto);
    assert_eq!(mode(PolicyEffect::Deny), RuleMode::Off);
}

// ── The breaker ────────────────────────────────

#[test]
fn a_scope_binding_tighter_than_the_requested_limit_is_the_breaker() {
    let budget = budget_from(Some(10.0), Some(12.0), true).expect("a cap");
    assert_eq!(
        budget.readout(),
        "— / 12.00",
        "the meter runs to the developer's limit"
    );
    assert_eq!(budget.breaker().as_deref(), Some("STOP AT 10.00"));
    assert_eq!(budget.tick_per_mille(), Some(833));
}

#[test]
fn an_unbound_or_equal_cap_is_the_whole_meter_with_no_tick() {
    // The developer's own limit was the tighter one: nothing to mark.
    let local = budget_from(Some(12.0), Some(12.0), false).expect("a cap");
    assert_eq!(local.readout(), "— / 12.00");
    assert_eq!(local.breaker(), None);
    // A scope caps a session that asked for no limit of its own: the
    // governed cap is the meter, and there is no local limit for the
    // scope to be tighter than.
    let scope_only = budget_from(Some(10.0), None, true).expect("a cap");
    assert_eq!(scope_only.readout(), "— / 10.00");
    assert_eq!(scope_only.breaker(), None);
    // A stale binding flag with nothing tightened draws no tick either.
    let stale = budget_from(Some(12.0), Some(12.0), true).expect("a cap");
    assert_eq!(stale.breaker(), None);
    assert_eq!(
        budget_from(None, Some(12.0), true),
        None,
        "no cap, no meter"
    );
    assert_eq!(
        budget_from(Some(0.0), None, true),
        None,
        "an exhausted cap is no meter to fill"
    );
}

// ── The rail bound live ────────────────

/// A project with no governance rule is still governed — by the
/// profile — so its charter is the profile's own defaults as rows, not
/// the section's empty state.
#[test]
fn a_project_with_no_rule_seeds_the_profiles_default_rows() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let model = derive(
        dir.path(),
        Some(&dir.path().join("costs.json")),
        PermissionProfile::Standard,
        false,
        None,
    );
    assert_eq!(
        rows(&model),
        vec![
            ("Edit".to_owned(), RuleMode::Ask),
            ("Run tests".to_owned(), RuleMode::Auto),
            ("git push".to_owned(), RuleMode::Ask),
        ],
        "Standard: reads run free, writes and commands ask"
    );
    assert_eq!(
        model.budget, None,
        "no roster and no limit asked: nothing caps the session"
    );
}

#[test]
fn the_default_rows_wear_each_profiles_tiers_and_all_auto_under_the_loop() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let modes = |profile, workflow| -> Vec<RuleMode> {
        rows(&derive(
            dir.path(),
            Some(&dir.path().join("costs.json")),
            profile,
            workflow,
            None,
        ))
        .into_iter()
        .map(|(_, mode)| mode)
        .collect()
    };
    use RuleMode::{Ask, Auto};
    assert_eq!(modes(PermissionProfile::Cautious, false), [Ask, Ask, Ask]);
    assert_eq!(modes(PermissionProfile::Trusted, false), [Auto, Auto, Ask]);
    assert_eq!(modes(PermissionProfile::Yolo, false), [Auto, Auto, Auto]);
    assert_eq!(
        modes(PermissionProfile::Standard, true),
        [Auto, Auto, Auto],
        "workflow mode auto-approves the standard operations by kind"
    );
}

/// The mode word is what the agent block says after the model: the
/// loop's, or the profile's.
#[test]
fn the_mode_word_is_the_loops_or_the_profiles() {
    assert_eq!(
        mode_word(PermissionProfile::Standard, true),
        "Workflow mode"
    );
    assert_eq!(
        mode_word(PermissionProfile::Standard, false),
        "Standard mode"
    );
    assert_eq!(mode_word(PermissionProfile::Trusted, false), "Trusted mode");
    let dir = tempfile::tempdir().expect("a temp dir");
    let model = derive(
        dir.path(),
        Some(&dir.path().join("costs.json")),
        PermissionProfile::Standard,
        true,
        None,
    );
    assert_eq!(model.mode, "Workflow mode");
}

/// The governed cap without an env var: a roster's session cap binds
/// the meter with nothing exported and nothing asked for locally.
#[test]
fn a_roster_cap_binds_the_meter_with_no_env_var_and_no_local_limit() {
    let dir = tempfile::tempdir().expect("a temp dir");
    roster_with(
        dir.path(),
        vec![BudgetRule::new(BudgetScope::Session, BudgetPeriod::Session).limit(2.0)],
    );
    let model = derive(
        dir.path(),
        Some(&dir.path().join("costs.json")),
        PermissionProfile::Standard,
        false,
        None,
    );
    let budget = model.budget.expect("the roster's cap binds");
    assert_eq!(budget.readout(), "— / 2.00");
    assert_eq!(
        budget.breaker(),
        None,
        "no local limit for the scope to tighten"
    );
}

/// The same roster over a developer who asked for more: the meter runs
/// to their limit and the breaker sits at the governed cap.
#[test]
fn a_roster_cap_tighter_than_the_requested_limit_draws_the_breaker() {
    let dir = tempfile::tempdir().expect("a temp dir");
    roster_with(
        dir.path(),
        vec![BudgetRule::new(BudgetScope::Session, BudgetPeriod::Session).limit(2.0)],
    );
    let model = derive(
        dir.path(),
        Some(&dir.path().join("costs.json")),
        PermissionProfile::Standard,
        false,
        Some(12.0),
    );
    let budget = model.budget.expect("a cap");
    assert_eq!(budget.readout(), "— / 12.00");
    assert_eq!(budget.breaker().as_deref(), Some("STOP AT 2.00"));
}

/// No roster, a requested limit: the developer's own cap, whole.
#[test]
fn a_requested_limit_with_no_roster_is_the_whole_meter() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let model = derive(
        dir.path(),
        Some(&dir.path().join("costs.json")),
        PermissionProfile::Standard,
        false,
        Some(5.0),
    );
    let budget = model.budget.expect("a cap");
    assert_eq!(budget.readout(), "— / 5.00");
    assert_eq!(budget.breaker(), None);
}

// ── The charter ───────────────────────────────

/// The reader's own rules follow the policy's rows, wearing their
/// effects, and a scope their record supports is an offer beside them.
#[test]
fn the_readers_delegations_seed_rows_and_a_clean_record_seeds_an_offer() {
    use magistr_engine::policy::{Charter, PolicyCondition, PolicyRule};

    let dir = tempfile::tempdir().expect("a temp dir");
    let mut charter = Charter::default();
    charter.delegate(
        PolicyRule::new(
            "delegate:edit:crates/x",
            PolicyCondition::tool("Edit"),
            PolicyEffect::Allow,
        )
        .describe("Edit \u{b7} crates/x"),
    );
    charter.delegate(PolicyRule::new(
        "refuse:bash:rm",
        PolicyCondition::command("rm*"),
        PolicyEffect::Deny,
    ));
    for _ in 0..Charter::OFFER_AFTER {
        charter.record(
            "bash:cargo",
            "Bash \u{b7} cargo",
            &PolicyCondition::command("cargo*"),
            true,
        );
    }
    crate::permissions::CharterStore::at(dir.path())
        .save(&charter)
        .expect("save the charter");

    let model = derive(
        dir.path(),
        Some(&dir.path().join("costs.json")),
        PermissionProfile::Standard,
        false,
        None,
    );
    assert_eq!(
        rows(&model),
        vec![
            ("Edit".to_owned(), RuleMode::Ask),
            ("Run tests".to_owned(), RuleMode::Auto),
            ("git push".to_owned(), RuleMode::Ask),
            ("Edit \u{b7} crates/x".to_owned(), RuleMode::Auto),
            ("refuse:bash:rm".to_owned(), RuleMode::Off),
        ],
        "the reader's rules after the profile's, an undescribed one by its id"
    );
    let offers = &model.offers;
    assert_eq!(offers.len(), 1);
    assert_eq!(offers[0].key(), "bash:cargo");
    assert_eq!(offers[0].subject(), "Bash \u{b7} cargo");
    assert_eq!(offers[0].tally(), "allowed 5 of 5");
}
