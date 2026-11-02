use super::*;
use crate::permissions::load_entries;
use tempfile::TempDir;

fn identity(name: &str, email: Option<&str>) -> Identity {
    Identity {
        name: name.to_string(),
        email: email.map(String::from),
        provider: None,
    }
}

#[test]
fn an_allowed_edit_is_recorded_with_the_actor() {
    let dir = TempDir::new().unwrap();
    let log = dir.path().join("permissions.log");

    audit_roster_edit(
        true,
        RosterChange::Add(PersonRole::Developer),
        "bob@example.dev",
        &identity("Alice", Some("alice@example.dev")),
        PersonRole::Owner,
        Some(&log),
        &AuditRetention::default(),
    );

    let entries = load_entries(&log).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].tool_name, "people_roster");
    assert!(entries[0].allowed);
    assert_eq!(entries[0].actor.as_deref(), Some("alice@example.dev"));
    assert_eq!(entries[0].risk, RiskLevel::Medium);
    assert!(
        entries[0]
            .reason
            .contains("add 'bob@example.dev' as developer")
    );
}

#[test]
fn a_refused_ownership_grab_is_recorded_as_high_risk_and_denied() {
    let dir = TempDir::new().unwrap();
    let log = dir.path().join("permissions.log");

    audit_roster_edit(
        false,
        RosterChange::SetRole {
            from: PersonRole::Admin,
            to: PersonRole::Owner,
        },
        "mallory@example.dev",
        &identity("Mallory", Some("mallory@example.dev")),
        PersonRole::Admin,
        Some(&log),
        &AuditRetention::default(),
    );

    let entries = load_entries(&log).unwrap();
    assert_eq!(entries.len(), 1);
    assert!(!entries[0].allowed);
    assert_eq!(entries[0].risk, RiskLevel::High);
    assert!(entries[0].reason.contains("refused"));
    assert!(entries[0].reason.contains("from admin to owner"));
    assert_eq!(entries[0].actor.as_deref(), Some("mallory@example.dev"));
}

#[test]
fn retention_bounds_are_applied_after_the_append() {
    let dir = TempDir::new().unwrap();
    let log = dir.path().join("permissions.log");
    let retention = AuditRetention {
        days: None,
        max_entries: Some(2),
    };
    let alice = identity("Alice", Some("alice@example.dev"));

    for id in ["a", "b", "c"] {
        audit_roster_edit(
            true,
            RosterChange::Add(PersonRole::Viewer),
            id,
            &alice,
            PersonRole::Owner,
            Some(&log),
            &retention,
        );
    }

    let entries = load_entries(&log).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].input_summary.as_deref(), Some("b"));
    assert_eq!(entries[1].input_summary.as_deref(), Some("c"));
}

#[test]
fn no_log_path_is_a_no_op() {
    audit_roster_edit(
        true,
        RosterChange::Add(PersonRole::Viewer),
        "bob",
        &identity("Alice", None),
        PersonRole::Owner,
        None,
        &AuditRetention::default(),
    );
}
