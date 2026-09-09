use super::*;
use crate::permissions::load_entries;
use crate::team::TeamMember;
use crate::team::directory::DirectoryConfig;
use std::path::PathBuf;
use tempfile::TempDir;

/// A roster where the machine's own identity holds `mine`, alongside an
/// Owner who is definitely somebody else. Writing the caller in by their
/// resolved id keeps the test free of environment mutation.
fn roster_with_me_as(dir: &Path, mine: TeamRole) -> Identity {
    let me = Identity::resolve(dir);
    let mut members = vec![TeamMember {
        id: "owner@example.dev".to_string(),
        name: Some("The Owner".to_string()),
        role: TeamRole::Owner,
        active: true,
    }];
    if mine != TeamRole::Owner {
        members.push(TeamMember {
            id: me.id().to_string(),
            name: Some(me.name.clone()),
            role: mine,
            active: true,
        });
    } else {
        members[0] = TeamMember {
            id: me.id().to_string(),
            name: Some(me.name.clone()),
            role: TeamRole::Owner,
            active: true,
        };
    }
    TeamConfig {
        budgets: Vec::new(),
        policy: Default::default(),
        version: 1,
        default_role: TeamRole::Viewer,
        identity_provider: None,
        members,
        directory: DirectoryConfig::default(),
    }
    .save(dir)
    .unwrap();
    me
}

fn log_path(dir: &TempDir) -> PathBuf {
    dir.path().join("permissions.log")
}

fn add(role: TeamRole) -> impl FnOnce(&TeamConfig) -> Result<RosterChange, MagistrError> {
    move |_| Ok(RosterChange::Add(role))
}

fn add_member(
    id: &str,
    role: TeamRole,
) -> impl FnOnce(&mut TeamConfig) -> Result<(), MagistrError> {
    let id = id.to_string();
    move |config: &mut TeamConfig| {
        config.add_member(TeamMember {
            id,
            name: None,
            role,
            active: true,
        })
    }
}

#[test]
fn an_admin_may_add_a_developer() {
    let tmp = TempDir::new().unwrap();
    roster_with_me_as(tmp.path(), TeamRole::Admin);
    let log = log_path(&tmp);

    let config = edit_roster(
        tmp.path(),
        "dev@example.dev",
        add(TeamRole::Developer),
        add_member("dev@example.dev", TeamRole::Developer),
        Some(&log),
        &AuditRetention::default(),
    )
    .unwrap();

    assert_eq!(config.role_of("dev@example.dev"), Some(TeamRole::Developer));
    let entries = load_entries(&log).unwrap();
    assert_eq!(entries.len(), 1);
    assert!(entries[0].allowed);
}

#[test]
fn an_admin_may_not_promote_themself_to_owner() {
    let tmp = TempDir::new().unwrap();
    let me = roster_with_me_as(tmp.path(), TeamRole::Admin);
    let log = log_path(&tmp);
    let my_id = me.id().to_string();

    let err = edit_roster(
        tmp.path(),
        &my_id,
        |config| {
            Ok(RosterChange::SetRole {
                from: config.role_of(&my_id).unwrap(),
                to: TeamRole::Owner,
            })
        },
        |config| config.set_role(&my_id, TeamRole::Owner),
        Some(&log),
        &AuditRetention::default(),
    )
    .unwrap_err();

    assert!(err.to_string().contains("transfer ownership"), "{err}");

    // The roster on disk is untouched…
    let config = TeamConfig::load(tmp.path()).unwrap().unwrap();
    assert_eq!(config.role_of(me.id()), Some(TeamRole::Admin));

    // …and the attempt is on the record.
    let entries = load_entries(&log).unwrap();
    assert_eq!(entries.len(), 1);
    assert!(!entries[0].allowed);
    assert_eq!(entries[0].actor.as_deref(), Some(me.id()));
}

#[test]
fn an_admin_may_not_demote_the_owner() {
    let tmp = TempDir::new().unwrap();
    roster_with_me_as(tmp.path(), TeamRole::Admin);
    let log = log_path(&tmp);

    let err = edit_roster(
        tmp.path(),
        "owner@example.dev",
        |config| {
            Ok(RosterChange::SetRole {
                from: config.role_of("owner@example.dev").unwrap(),
                to: TeamRole::Viewer,
            })
        },
        |config| config.set_role("owner@example.dev", TeamRole::Viewer),
        Some(&log),
        &AuditRetention::default(),
    )
    .unwrap_err();

    assert!(err.to_string().contains("grant or alter Owner"), "{err}");
}

#[test]
fn a_viewer_may_not_edit_the_roster_at_all() {
    let tmp = TempDir::new().unwrap();
    roster_with_me_as(tmp.path(), TeamRole::Viewer);
    let log = log_path(&tmp);

    let err = edit_roster(
        tmp.path(),
        "dev@example.dev",
        add(TeamRole::Developer),
        add_member("dev@example.dev", TeamRole::Developer),
        Some(&log),
        &AuditRetention::default(),
    )
    .unwrap_err();

    assert!(err.to_string().contains("may not edit the roster"), "{err}");
    assert!(
        TeamConfig::load(tmp.path())
            .unwrap()
            .unwrap()
            .role_of("dev@example.dev")
            .is_none()
    );
}

#[test]
fn an_owner_may_transfer_ownership() {
    let tmp = TempDir::new().unwrap();
    roster_with_me_as(tmp.path(), TeamRole::Owner);
    let log = log_path(&tmp);

    let config = edit_roster(
        tmp.path(),
        "heir@example.dev",
        add(TeamRole::Owner),
        add_member("heir@example.dev", TeamRole::Owner),
        Some(&log),
        &AuditRetention::default(),
    )
    .unwrap();

    assert_eq!(config.role_of("heir@example.dev"), Some(TeamRole::Owner));
    let entries = load_entries(&log).unwrap();
    assert!(entries[0].allowed);
}

#[test]
fn the_integrity_rules_still_apply_to_an_owner() {
    let tmp = TempDir::new().unwrap();
    let me = roster_with_me_as(tmp.path(), TeamRole::Owner);
    let my_id = me.id().to_string();
    let log = log_path(&tmp);

    // Authorized (Owner) but refused by TeamConfig's last-Owner rule.
    let err = edit_roster(
        tmp.path(),
        &my_id,
        |config| Ok(RosterChange::Remove(config.role_of(&my_id).unwrap())),
        |config| config.remove_member(&my_id),
        Some(&log),
        &AuditRetention::default(),
    )
    .unwrap_err();

    assert!(err.to_string().contains("last Owner"), "{err}");
}

#[test]
fn a_missing_roster_asks_for_init() {
    let tmp = TempDir::new().unwrap();
    let err = edit_roster(
        tmp.path(),
        "dev@example.dev",
        add(TeamRole::Developer),
        add_member("dev@example.dev", TeamRole::Developer),
        None,
        &AuditRetention::default(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("magistr team init"), "{err}");
}
