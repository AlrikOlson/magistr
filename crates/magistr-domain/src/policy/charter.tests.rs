use alloc::vec;

use super::*;

use crate::policy::PolicyEngine;

const KEY: &str = "edit:crates/magistr-desktop";
const SUBJECT: &str = "Edit \u{b7} crates/magistr-desktop";

fn scope() -> PolicyCondition {
    PolicyCondition::all_of(vec![
        PolicyCondition::tool("Edit"),
        PolicyCondition::any_of(vec![
            PolicyCondition::path("crates/magistr-desktop/*"),
            PolicyCondition::path("*/crates/magistr-desktop/*"),
        ]),
    ])
}

fn edit(path: &str) -> PolicyRequest {
    PolicyRequest::tool("Edit").with_path(path)
}

fn allowed(charter: &Charter, request: &PolicyRequest) -> PolicyEffect {
    charter.apply(PolicyEffect::RequireApproval, request).effect
}

#[test]
fn an_empty_charter_leaves_the_local_answer_unnamed() {
    let charter = Charter::default();
    assert!(charter.is_empty());
    let decision = charter.apply(PolicyEffect::RequireApproval, &edit("src/main.rs"));
    assert_eq!(decision.effect, PolicyEffect::RequireApproval);
    assert_eq!(decision.binding, None);
    assert!(decision.matched.is_empty());
    assert!(charter.offers().is_empty());
}

#[test]
fn a_delegated_allow_lowers_an_ask_under_its_scope_and_names_itself() {
    let mut charter = Charter::default();
    let rule = PolicyRule::new(Charter::delegation_id(KEY), scope(), PolicyEffect::Allow);
    assert!(charter.delegate(rule.clone()));
    assert!(
        !charter.delegate(rule),
        "the same rule again changes nothing"
    );

    let inside = charter.apply(
        PolicyEffect::RequireApproval,
        &edit("/repo/crates/magistr-desktop/src/view.rs"),
    );
    assert_eq!(inside.effect, PolicyEffect::Allow);
    assert_eq!(
        inside.binding.as_deref(),
        Some("delegate:edit:crates/magistr-desktop")
    );
    // The agent's relative spelling of the same place.
    assert_eq!(
        allowed(&charter, &edit("crates/magistr-desktop/src/view.rs")),
        PolicyEffect::Allow
    );
    // Outside the scope the profile's ask stands.
    assert_eq!(
        allowed(&charter, &edit("crates/magistr-cockpit/src/lib.rs")),
        PolicyEffect::RequireApproval
    );
    assert_eq!(
        allowed(
            &charter,
            &PolicyRequest::tool("Bash").with_command("rm -rf crates/magistr-desktop")
        ),
        PolicyEffect::RequireApproval,
        "a scope is a tool over a path, not the path alone"
    );
}

#[test]
fn the_tightest_delegation_wins_whatever_order_they_were_written_in() {
    let mut charter = Charter::default();
    charter.delegate(PolicyRule::new("delegate:x", scope(), PolicyEffect::Allow));
    charter.delegate(PolicyRule::new("refuse:x", scope(), PolicyEffect::Deny));
    let request = edit("crates/magistr-desktop/src/view.rs");
    let decision = charter.apply(PolicyEffect::Allow, &request);
    assert_eq!(decision.effect, PolicyEffect::Deny);
    assert_eq!(decision.binding.as_deref(), Some("refuse:x"));
    assert_eq!(decision.matched, vec!["delegate:x", "refuse:x"]);

    assert!(charter.revoke("refuse:x"));
    assert!(!charter.revoke("refuse:x"), "already gone");
    assert_eq!(allowed(&charter, &request), PolicyEffect::Allow);
}

#[test]
fn a_written_condition_raises_the_profiles_allow_to_an_ask() {
    let mut charter = Charter::default();
    charter.delegate(
        PolicyRule::new("ask:x", scope(), PolicyEffect::RequireApproval)
            .describe("allow, stop if other rows change"),
    );
    let decision = charter.apply(
        PolicyEffect::Allow,
        &edit("crates/magistr-desktop/src/view.rs"),
    );
    assert_eq!(decision.effect, PolicyEffect::RequireApproval);
    assert_eq!(
        charter.delegation("ask:x").map(|r| r.description.as_str()),
        Some("allow, stop if other rows change")
    );
}

/// The lattice: the roster folds its maximum over the widened local
/// decision, so a delegated Allow under a roster Deny is a Deny.
#[test]
fn the_roster_tightens_over_every_delegation() {
    let mut charter = Charter::default();
    charter.delegate(PolicyRule::new(
        Charter::delegation_id("edit:*"),
        PolicyCondition::tool("Edit"),
        PolicyEffect::Allow,
    ));
    let roster = PolicyEngine::new(vec![PolicyRule::new(
        "roster-is-precious",
        PolicyCondition::path("*team.json"),
        PolicyEffect::Deny,
    )]);
    let precious = edit("/repo/.magistr/team.json");
    let widened = charter.apply(PolicyEffect::RequireApproval, &precious);
    assert_eq!(
        widened.effect,
        PolicyEffect::Allow,
        "the charter alone would allow"
    );
    let governed = roster.tighten(widened.effect, &precious);
    assert_eq!(governed.effect, PolicyEffect::Deny);
    assert_eq!(governed.binding.as_deref(), Some("roster-is-precious"));

    // And where the roster says nothing, the widening holds.
    let plain = edit("/repo/src/main.rs");
    assert_eq!(
        roster
            .tighten(
                charter.apply(PolicyEffect::RequireApproval, &plain).effect,
                &plain
            )
            .effect,
        PolicyEffect::Allow
    );
}

#[test]
fn answers_tally_per_scope() {
    let mut charter = Charter::default();
    charter.record(KEY, SUBJECT, &scope(), true);
    charter.record(KEY, SUBJECT, &scope(), true);
    charter.record(
        "run:cargo",
        "Run \u{b7} cargo",
        &PolicyCondition::command("cargo*"),
        false,
    );
    let tally = charter.tally(KEY).expect("the edit scope");
    assert_eq!((tally.allowed, tally.denied), (2, 0));
    assert_eq!(tally.subject, SUBJECT);
    assert_eq!(tally.condition, scope());
    assert_eq!(charter.tally("run:cargo").map(|t| t.denied), Some(1));
    assert_eq!(charter.tally("nowhere"), None);
    assert_eq!(charter.tallies.len(), 2);
}

#[test]
fn a_clean_scope_past_the_threshold_is_offered_until_it_is_covered() {
    let mut charter = Charter::default();
    for _ in 0..Charter::OFFER_AFTER - 1 {
        charter.record(KEY, SUBJECT, &scope(), true);
    }
    assert!(charter.offers().is_empty(), "one short of the threshold");
    charter.record(KEY, SUBJECT, &scope(), true);
    let offers = charter.offers();
    assert_eq!(offers.len(), 1);
    assert_eq!(offers[0].key, KEY);
    assert_eq!(offers[0].subject, SUBJECT);
    assert_eq!((offers[0].allowed, offers[0].asked), (5, 5));

    // A denied scope is never offered, however many allows it has.
    charter.record(
        "run:cargo",
        "Run \u{b7} cargo",
        &PolicyCondition::command("cargo*"),
        false,
    );
    for _ in 0..Charter::OFFER_AFTER {
        charter.record(
            "run:cargo",
            "Run \u{b7} cargo",
            &PolicyCondition::command("cargo*"),
            true,
        );
    }
    assert_eq!(charter.offers().len(), 1);

    // A scope the reader already wrote a rule over — of any effect — is
    // not offered again.
    charter.delegate(PolicyRule::new(
        "ask:x",
        scope(),
        PolicyEffect::RequireApproval,
    ));
    assert!(charter.offers().is_empty());
}

#[test]
fn accepting_an_offer_writes_the_allow_rule_the_scope_is_then_decided_by() {
    let mut charter = Charter::default();
    assert_eq!(charter.accept(KEY), None, "nothing was answered under it");
    for _ in 0..Charter::OFFER_AFTER {
        charter.record(KEY, SUBJECT, &scope(), true);
    }
    let rule = charter.accept(KEY).expect("an accepted offer");
    assert_eq!(rule.id, "delegate:edit:crates/magistr-desktop");
    assert_eq!(rule.description, SUBJECT);
    assert_eq!(rule.effect, PolicyEffect::Allow);
    assert_eq!(rule.condition, scope());
    assert_eq!(charter.delegation(&rule.id), Some(&rule));
    assert_eq!(
        allowed(&charter, &edit("crates/magistr-desktop/src/view.rs")),
        PolicyEffect::Allow
    );
    assert!(
        charter.offers().is_empty(),
        "accepted, so no longer offered"
    );
}

#[test]
fn the_charter_round_trips_through_json_and_an_empty_one_is_bare() {
    assert_eq!(
        serde_json::to_string(&Charter::default()).expect("serialize"),
        "{}"
    );
    let mut charter = Charter::default();
    charter.record(KEY, SUBJECT, &scope(), true);
    charter.delegate(
        PolicyRule::new(Charter::delegation_id(KEY), scope(), PolicyEffect::Allow)
            .describe(SUBJECT),
    );
    let json = serde_json::to_string_pretty(&charter).expect("serialize");
    let back: Charter = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, charter);
}
