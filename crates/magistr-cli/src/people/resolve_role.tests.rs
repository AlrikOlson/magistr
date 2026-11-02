use super::super::person::Person;
use super::*;
use crate::people::directory::DirectoryConfig;

fn identity(name: &str, email: Option<&str>) -> Identity {
    Identity {
        name: name.to_string(),
        email: email.map(String::from),
        provider: None,
    }
}

fn team() -> PeopleConfig {
    PeopleConfig {
        budgets: Vec::new(),
        policy: Default::default(),
        version: 1,
        default_role: PersonRole::Viewer,
        identity_provider: None,
        members: vec![
            Person {
                id: "alice@example.dev".to_string(),
                name: Some("Alice".to_string()),
                role: PersonRole::Owner,
                active: true,
            },
            Person {
                id: "dave@example.dev".to_string(),
                name: None,
                role: PersonRole::Developer,
                active: true,
            },
        ],
        directory: DirectoryConfig::default(),
    }
}

#[test]
fn solo_mode_is_owner() {
    let id = identity("anyone", None);
    assert_eq!(resolve_role(None, &id, false), PersonRole::Owner);
    // Solo mode wins even under CI.
    assert_eq!(resolve_role(None, &id, true), PersonRole::Owner);
}

#[test]
fn roster_member_gets_their_role() {
    let config = team();
    let dave = identity("Dave", Some("dave@example.dev"));
    assert_eq!(
        resolve_role(Some(&config), &dave, false),
        PersonRole::Developer
    );
    // A roster match beats CI detection.
    assert_eq!(
        resolve_role(Some(&config), &dave, true),
        PersonRole::Developer
    );
}

#[test]
fn unknown_identity_under_ci_is_ci_role() {
    let config = team();
    let bot = identity("runner", None);
    assert_eq!(resolve_role(Some(&config), &bot, true), PersonRole::Ci);
}

#[test]
fn unknown_identity_gets_default_role() {
    let config = team();
    let stranger = identity("mallory", Some("mallory@example.dev"));
    assert_eq!(
        resolve_role(Some(&config), &stranger, false),
        PersonRole::Viewer
    );
}
