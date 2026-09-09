use super::*;

#[test]
fn a_renewal_inside_the_ttl_is_ready() {
    assert_eq!(NodeHealth::from_lease(0, 60), NodeHealth::Ready);
    assert_eq!(NodeHealth::from_lease(59, 60), NodeHealth::Ready);
    assert_eq!(NodeHealth::from_lease(60, 60), NodeHealth::Ready);
}

#[test]
fn a_missed_renewal_inside_the_allowance_is_stale() {
    assert_eq!(NodeHealth::from_lease(61, 60), NodeHealth::Stale);
    assert_eq!(NodeHealth::from_lease(180, 60), NodeHealth::Stale);
}

#[test]
fn past_the_allowance_the_node_is_unreachable() {
    assert_eq!(NodeHealth::from_lease(181, 60), NodeHealth::Unreachable);
    assert_eq!(NodeHealth::from_lease(86_400, 60), NodeHealth::Unreachable);
}

#[test]
fn clock_skew_ahead_of_the_control_plane_does_not_kill_a_node() {
    // The node's clock runs ahead: its last renewal is "in the future".
    assert_eq!(NodeHealth::from_lease(-30, 60), NodeHealth::Ready);
}

#[test]
fn a_zero_ttl_is_floored_rather_than_dividing_the_fleet_by_zero() {
    assert_eq!(NodeHealth::from_lease(1, 0), NodeHealth::Ready);
    assert_eq!(NodeHealth::from_lease(4, 0), NodeHealth::Unreachable);
}

#[test]
fn an_enormous_ttl_does_not_overflow_the_allowance() {
    assert_eq!(NodeHealth::from_lease(1, i64::MAX), NodeHealth::Ready);
    assert_eq!(
        NodeHealth::from_lease(i64::MAX, i64::MAX),
        NodeHealth::Ready
    );
}

#[test]
fn only_unreachable_is_not_live() {
    assert!(NodeHealth::Ready.is_live());
    assert!(NodeHealth::Stale.is_live());
    assert!(!NodeHealth::Unreachable.is_live());
}

#[test]
fn health_round_trips_through_its_wire_spelling() {
    for health in [
        NodeHealth::Ready,
        NodeHealth::Stale,
        NodeHealth::Unreachable,
    ] {
        let json = serde_json::to_string(&health).expect("serializable");
        assert_eq!(json, format!("\"{}\"", health.as_str()));
    }
}
