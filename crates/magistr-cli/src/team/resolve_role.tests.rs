use super::super::team_member::TeamMember;
use super::*;
use crate::team::directory::DirectoryConfig;

fn identity(name: &str, email: Option<&str>) -> Identity {
    Identity {
        name: name.to_string(),
        email: email.map(String::from),
        provider: None,
    }
}

fn team() -> TeamConfig {
    TeamConfig {
        budgets: Vec::new(),
        policy: Default::default(),
        version: 1,
        default_role: TeamRole::Viewer,
        identity_provider: None,
        members: vec![
            TeamMember {
                id: "alice@example.dev".to_string(),
                name: Some("Alice".to_string()),
                role: TeamRole::Owner,
                active: true,
            },
            TeamMember {
                id: "dave@example.dev".to_string(),
                name: None,
                role: TeamRole::Developer,
                active: true,
            },
        ],
        directory: DirectoryConfig::default(),
    }
}

#[test]
fn solo_mode_is_owner() {
    let id = identity("anyone", None);
    assert_eq!(resolve_role(None, &id, false), TeamRole::Owner);
    // Solo mode wins even under CI.
    assert_eq!(resolve_role(None, &id, true), TeamRole::Owner);
}

#[test]
fn roster_member_gets_their_role() {
    let config = team();
    let dave = identity("Dave", Some("dave@example.dev"));
    assert_eq!(
        resolve_role(Some(&config), &dave, false),
        TeamRole::Developer
    );
    // A roster match beats CI detection.
    assert_eq!(
        resolve_role(Some(&config), &dave, true),
        TeamRole::Developer
    );
}

#[test]
fn unknown_identity_under_ci_is_ci_role() {
    let config = team();
    let bot = identity("runner", None);
    assert_eq!(resolve_role(Some(&config), &bot, true), TeamRole::Ci);
}

#[test]
fn unknown_identity_gets_default_role() {
    let config = team();
    let stranger = identity("mallory", Some("mallory@example.dev"));
    assert_eq!(
        resolve_role(Some(&config), &stranger, false),
        TeamRole::Viewer
    );
}
