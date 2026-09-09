use super::*;
use crate::policy::governance_policy::GovernancePolicy;

fn at(secs: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(1_800_000_000 + secs, 0).expect("valid timestamp")
}

fn node(id: &str, now: DateTime<Utc>) -> FleetNode {
    FleetNode::new(id, format!("host-{id}"), "0.1.0", now)
}

fn published(generation: u64) -> PolicyBundle {
    PolicyBundle::seal(
        generation,
        GovernancePolicy {
            tool_denylist: vec!["Bash".into()],
            ..GovernancePolicy::default()
        },
        vec![],
    )
}

// ── Registration ────────────────────────────────────────────────────

#[test]
fn a_new_fleet_is_empty_with_nothing_published() {
    let fleet = FleetRegistry::new();

    assert!(fleet.is_empty());
    assert_eq!(fleet.len(), 0);
    assert_eq!(fleet.bundle().generation, 0);
    assert_eq!(
        fleet.lease_ttl_secs(),
        FleetRegistry::DEFAULT_LEASE_TTL_SECS
    );
}

#[test]
fn registering_a_node_hands_back_the_bundle_it_must_apply() {
    let mut fleet = FleetRegistry::new();
    fleet.publish(published(1)).expect("first publish");

    let handed = fleet.register(node("a", at(0))).clone();

    assert_eq!(handed.generation, 1);
    assert_eq!(fleet.len(), 1);
    assert!(fleet.node("a").is_some());
}

#[test]
fn re_registering_an_id_replaces_the_record_and_clears_its_claim() {
    let mut fleet = FleetRegistry::new();
    fleet.publish(published(1)).expect("first publish");
    let digest = fleet.bundle().digest.clone();
    fleet.register(node("a", at(0)));
    fleet
        .heartbeat("a", at(10), 1, &digest)
        .expect("known node");
    assert!(fleet.status(at(10)).drifted.is_empty());

    // The node restarts on a new version and registers again.
    let mut restarted = node("a", at(20));
    restarted.version = "0.2.0".into();
    fleet.register(restarted);

    assert_eq!(fleet.len(), 1);
    assert_eq!(fleet.status(at(20)).drifted, ["a"]);
}

#[test]
fn a_forgotten_node_leaves_the_fleet() {
    let mut fleet = FleetRegistry::new();
    fleet.register(node("a", at(0)));

    assert!(fleet.forget("a"));
    assert!(!fleet.forget("a"));
    assert!(fleet.is_empty());
}

// ── Publishing ──────────────────────────────────────────────────────

#[test]
fn publishing_a_newer_generation_replaces_the_current_bundle() {
    let mut fleet = FleetRegistry::new();

    fleet.publish(published(1)).expect("first publish");
    assert_eq!(fleet.bundle().generation, 1);

    fleet.publish(published(2)).expect("second publish");
    assert_eq!(fleet.bundle().generation, 2);
}

#[test]
fn republishing_the_same_generation_is_refused() {
    let mut fleet = FleetRegistry::new();
    fleet.publish(published(3)).expect("first publish");

    let err = fleet.publish(published(3)).expect_err("not newer");

    assert!(matches!(err, EngineError::Precondition(_)));
    assert_eq!(fleet.bundle().generation, 3);
}

#[test]
fn rolling_back_to_a_looser_generation_is_refused() {
    let mut fleet = FleetRegistry::new();
    fleet.publish(published(5)).expect("first publish");

    let loose = PolicyBundle::seal(4, GovernancePolicy::default(), vec![]);
    fleet.publish(loose).expect_err("older generation");

    assert_eq!(fleet.bundle().generation, 5);
    assert_eq!(fleet.bundle().policy.tool_denylist, ["Bash"]);
}

#[test]
fn generation_zero_can_never_be_published() {
    let mut fleet = FleetRegistry::new();

    fleet
        .publish(PolicyBundle::empty())
        .expect_err("generation 0 is the unpublished sentinel");

    assert!(!fleet.bundle().is_published());
}

#[test]
fn a_bundle_whose_digest_does_not_match_its_contents_is_refused() {
    let mut fleet = FleetRegistry::new();
    let mut tampered = published(1);
    tampered.policy.tool_denylist.clear();

    let err = fleet.publish(tampered).expect_err("digest mismatch");

    assert!(matches!(err, EngineError::Precondition(_)));
    assert_eq!(fleet.bundle().generation, 0);
}

// ── Heartbeats ──────────────────────────────────────────────────────

#[test]
fn a_heartbeat_from_an_unknown_node_is_refused_rather_than_registering_it() {
    let mut fleet = FleetRegistry::new();

    let err = fleet
        .heartbeat("ghost", at(0), 0, "")
        .expect_err("unknown node");

    assert!(matches!(err, EngineError::Precondition(_)));
    assert!(fleet.is_empty());
}

#[test]
fn a_heartbeat_behind_the_current_generation_gets_the_bundle_back() {
    let mut fleet = FleetRegistry::new();
    fleet.publish(published(2)).expect("publish");
    fleet.register(node("a", at(0)));

    let owed = fleet
        .heartbeat("a", at(30), 1, "fnv1a64:0000000000000001")
        .expect("known node");

    assert_eq!(owed.map(|b| b.generation), Some(2));
}

#[test]
fn a_heartbeat_on_the_current_bundle_is_owed_nothing() {
    let mut fleet = FleetRegistry::new();
    fleet.publish(published(2)).expect("publish");
    let digest = fleet.bundle().digest.clone();
    fleet.register(node("a", at(0)));

    let owed = fleet
        .heartbeat("a", at(30), 2, &digest)
        .expect("known node");

    assert!(owed.is_none());
}

#[test]
fn a_heartbeat_renews_the_lease_even_when_the_node_is_drifted() {
    let mut fleet = FleetRegistry::new();
    fleet.publish(published(2)).expect("publish");
    fleet.register(node("a", at(0)));

    fleet.heartbeat("a", at(200), 0, "").expect("known node");

    let status = fleet.status(at(210));
    assert_eq!(status.ready, 1);
    assert_eq!(status.drifted, ["a"]);
}

// ── Health and eviction ─────────────────────────────────────────────

#[test]
fn the_rollup_counts_each_node_by_its_lease_age() {
    let mut fleet = FleetRegistry::with_lease_ttl_secs(60);
    fleet.register(node("fresh", at(180)));
    fleet.register(node("slow", at(100)));
    fleet.register(node("gone", at(0)));

    let status = fleet.status(at(200));

    assert_eq!(status.total, 3);
    assert_eq!(status.ready, 1);
    assert_eq!(status.stale, 1);
    assert_eq!(status.unreachable, 1);
    assert_eq!(status.live(), 2);
}

#[test]
fn the_rollup_names_the_drifted_nodes_in_id_order() {
    let mut fleet = FleetRegistry::with_lease_ttl_secs(60);
    fleet.publish(published(1)).expect("publish");
    let digest = fleet.bundle().digest.clone();
    for id in ["c", "a", "b"] {
        fleet.register(node(id, at(0)));
    }
    fleet.heartbeat("b", at(0), 1, &digest).expect("known node");

    let status = fleet.status(at(0));

    assert_eq!(status.on_current, 1);
    assert_eq!(status.drifted, ["a", "c"]);
    assert!(!status.is_converged());
}

#[test]
fn a_fleet_all_on_the_current_bundle_and_all_reachable_has_converged() {
    let mut fleet = FleetRegistry::with_lease_ttl_secs(60);
    fleet.publish(published(1)).expect("publish");
    let digest = fleet.bundle().digest.clone();
    for id in ["a", "b"] {
        fleet.register(node(id, at(0)));
        fleet.heartbeat(id, at(0), 1, &digest).expect("known node");
    }

    assert!(fleet.status(at(30)).is_converged());
}

#[test]
fn eviction_drops_only_the_nodes_past_the_miss_allowance() {
    let mut fleet = FleetRegistry::with_lease_ttl_secs(60);
    fleet.register(node("gone", at(0)));
    fleet.register(node("slow", at(100)));
    fleet.register(node("fresh", at(180)));

    let evicted = fleet.evict_expired(at(200));

    assert_eq!(evicted, ["gone"]);
    assert_eq!(fleet.len(), 2);
    assert!(fleet.node("gone").is_none());
}

#[test]
fn eviction_on_a_healthy_fleet_drops_nothing() {
    let mut fleet = FleetRegistry::with_lease_ttl_secs(60);
    fleet.register(node("a", at(0)));

    assert!(fleet.evict_expired(at(30)).is_empty());
    assert_eq!(fleet.len(), 1);
}

#[test]
fn a_lease_ttl_of_zero_is_floored_rather_than_evicting_the_whole_fleet() {
    let fleet = FleetRegistry::with_lease_ttl_secs(0);

    assert_eq!(fleet.lease_ttl_secs(), 1);
}
