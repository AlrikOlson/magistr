use super::*;

#[test]
fn a_pending_permission_is_the_only_blocking_residency() {
    assert!(
        SessionResidency::PendingPermission {
            request_id: "req-1".into()
        }
        .is_blocking()
    );
    assert!(!SessionResidency::AgentProcess { pid: Some(42) }.is_blocking());
    assert!(!SessionResidency::Terminal { id: "t1".into() }.is_blocking());
    assert!(
        !SessionResidency::WorkspaceLock {
            workspace: "/tmp/w".into()
        }
        .is_blocking()
    );
}

#[test]
fn every_residency_names_itself_and_says_why_it_stays_behind() {
    let all = [
        SessionResidency::AgentProcess { pid: None },
        SessionResidency::Terminal { id: "t".into() },
        SessionResidency::WorkspaceLock {
            workspace: "w".into(),
        },
        SessionResidency::PendingPermission {
            request_id: "r".into(),
        },
    ];
    for residency in &all {
        assert!(!residency.kind().is_empty());
        assert!(!residency.reason().is_empty());
        // The description names the kind, so an operator reading one
        // line knows which of the four it is.
        assert!(residency.describe().contains(residency.kind()));
    }
}

#[test]
fn a_residency_round_trips_through_json() {
    let residency = SessionResidency::WorkspaceLock {
        workspace: "/srv/proj".into(),
    };
    let json = serde_json::to_string(&residency).expect("serialize");
    let parsed: SessionResidency = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(residency, parsed);
}

#[test]
fn the_kinds_are_distinct() {
    let kinds = [
        SessionResidency::AgentProcess { pid: None }.kind(),
        SessionResidency::Terminal { id: "t".into() }.kind(),
        SessionResidency::WorkspaceLock {
            workspace: "w".into(),
        }
        .kind(),
        SessionResidency::PendingPermission {
            request_id: "r".into(),
        }
        .kind(),
    ];
    for (i, a) in kinds.iter().enumerate() {
        for b in &kinds[i + 1..] {
            assert_ne!(a, b);
        }
    }
}
