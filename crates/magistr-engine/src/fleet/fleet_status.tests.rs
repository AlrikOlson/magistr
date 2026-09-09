use super::*;

fn converged() -> FleetStatus {
    FleetStatus {
        total: 3,
        ready: 3,
        stale: 0,
        unreachable: 0,
        generation: 5,
        digest: "fnv1a64:0000000000000005".into(),
        on_current: 3,
        drifted: vec![],
    }
}

#[test]
fn an_empty_fleet_is_trivially_converged() {
    assert!(FleetStatus::default().is_converged());
    assert_eq!(FleetStatus::default().live(), 0);
}

#[test]
fn a_fleet_all_on_the_current_bundle_has_converged() {
    assert!(converged().is_converged());
    assert_eq!(converged().live(), 3);
}

#[test]
fn a_stale_node_still_counts_as_live() {
    let status = FleetStatus {
        ready: 2,
        stale: 1,
        ..converged()
    };

    assert_eq!(status.live(), 3);
    assert!(status.is_converged());
}

#[test]
fn one_drifted_node_breaks_convergence() {
    let status = FleetStatus {
        on_current: 2,
        drifted: vec!["node-c".into()],
        ..converged()
    };

    assert!(!status.is_converged());
}

#[test]
fn an_unreachable_node_breaks_convergence_whatever_it_last_reported() {
    let status = FleetStatus {
        ready: 2,
        unreachable: 1,
        ..converged()
    };

    assert!(status.drifted.is_empty());
    assert!(!status.is_converged());
}
