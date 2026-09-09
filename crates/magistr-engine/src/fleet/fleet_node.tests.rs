use super::*;
use crate::policy::governance_policy::GovernancePolicy;

fn at(secs: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(1_800_000_000 + secs, 0).expect("valid timestamp")
}

fn node() -> FleetNode {
    FleetNode::new("node-a", "builder-01", "0.1.0", at(0))
}

fn bundle(generation: u64) -> PolicyBundle {
    PolicyBundle::seal(generation, GovernancePolicy::default(), vec![])
}

#[test]
fn a_new_node_holds_its_lease_and_runs_no_policy() {
    let node = node();

    assert_eq!(node.id, "node-a");
    assert_eq!(node.last_seen, at(0));
    assert_eq!(node.applied_generation, 0);
    assert!(node.applied_digest.is_empty());
    assert!(node.projects.is_empty());
}

#[test]
fn lease_age_grows_with_the_control_planes_clock() {
    let node = node();

    assert_eq!(node.lease_age_secs(at(0)), 0);
    assert_eq!(node.lease_age_secs(at(45)), 45);
}

#[test]
fn a_renewal_resets_the_lease_and_records_what_the_node_runs() {
    let mut node = node();
    let published = bundle(3);

    node.renew(at(300), published.generation, published.digest.clone());

    assert_eq!(node.lease_age_secs(at(300)), 0);
    assert!(node.is_current(&published));
}

#[test]
fn health_falls_through_ready_stale_and_unreachable_as_the_lease_ages() {
    let node = node();

    assert_eq!(node.health(at(30), 60), NodeHealth::Ready);
    assert_eq!(node.health(at(90), 60), NodeHealth::Stale);
    assert_eq!(node.health(at(600), 60), NodeHealth::Unreachable);
}

#[test]
fn a_node_that_never_applied_anything_is_not_current() {
    assert!(!node().is_current(&bundle(1)));
}

#[test]
fn a_node_on_an_older_generation_is_not_current() {
    let mut node = node();
    let old = bundle(4);
    node.renew(at(10), old.generation, old.digest.clone());

    assert!(node.is_current(&old));
    assert!(!node.is_current(&bundle(5)));
}

#[test]
fn the_right_generation_with_the_wrong_content_is_still_drifted() {
    let mut node = node();
    node.renew(at(10), 4, "fnv1a64:deadbeefdeadbeef");

    let published = bundle(4);
    assert_eq!(node.applied_generation, published.generation);
    assert!(!node.is_current(&published));
}

#[test]
fn a_node_carries_the_projects_it_serves() {
    let node = node().with_projects(vec!["magistr".into(), "workbench".into()]);

    assert_eq!(node.projects, ["magistr", "workbench"]);
}

#[test]
fn a_node_round_trips_through_json() {
    let node = node().with_projects(vec!["magistr".into()]);

    let json = serde_json::to_string(&node).expect("serializable");
    let back: FleetNode = serde_json::from_str(&json).expect("deserializable");

    assert_eq!(back, node);
}

// ── Cordon ───────────────────────────────────────────────────

#[test]
fn a_new_node_is_not_cordoned() {
    let node = FleetNode::new("n1", "n1.local", "1.0.0", at(0));
    assert!(!node.draining);
    assert!(node.accepts_work(at(1), 60));
}

#[test]
fn a_cordoned_node_is_healthy_and_still_takes_no_work() {
    let mut node = FleetNode::new("n1", "n1.local", "1.0.0", at(0));
    node.cordon();
    assert!(node.draining);
    assert_eq!(node.health(at(1), 60), NodeHealth::Ready);
    assert!(
        !node.accepts_work(at(1), 60),
        "a node draining for maintenance is healthy right up to the switch-off"
    );
}

#[test]
fn cordoning_is_idempotent_and_reversible() {
    let mut node = FleetNode::new("n1", "n1.local", "1.0.0", at(0));
    node.cordon();
    node.cordon();
    assert!(node.draining);
    node.uncordon();
    assert!(!node.draining);
    assert!(node.accepts_work(at(1), 60));
}

#[test]
fn an_unreachable_node_takes_no_work_whether_cordoned_or_not() {
    let node = FleetNode::new("n1", "n1.local", "1.0.0", at(0));
    assert_eq!(node.health(at(1000), 60), NodeHealth::Unreachable);
    assert!(!node.accepts_work(at(1000), 60));
}

#[test]
fn a_record_written_before_draining_existed_reads_as_uncordoned() {
    let json = r#"{
        "id": "n1",
        "hostname": "n1.local",
        "version": "1.0.0",
        "last_seen": "2027-01-15T00:00:00Z"
    }"#;
    let node: FleetNode = serde_json::from_str(json).expect("an e10a-era record");
    assert!(!node.draining);
    assert_eq!(node.capacity, FleetNode::DEFAULT_CAPACITY);
}
