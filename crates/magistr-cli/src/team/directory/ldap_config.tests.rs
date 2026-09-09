use super::*;

use std::cell::RefCell;
use std::collections::HashMap;

use crate::error::MagistrError;
use crate::team::TeamRole;
use crate::team::directory::{DirectoryOutcome, GroupRole, GroupSource};

// ── The filter ─────────────────────────────────────────────────────────

#[test]
fn the_subject_is_substituted_into_the_filter() {
    let config = LdapConfig::new("ldaps://ldap.example.com", "ou=people,dc=example,dc=com");
    assert_eq!(config.filter_for("ada"), "(uid=ada)");
}

/// A bare `*` in an unescaped filter matches every user in the base DN,
/// which turns a lookup for one person into a lookup for the directory.
#[test]
fn a_wildcard_subject_cannot_widen_the_search() {
    let config = LdapConfig::new("ldaps://ldap.example.com", "dc=example,dc=com");
    assert_eq!(config.filter_for("*"), "(uid=\\2a)");
}

/// The classic LDAP injection: close the filter, open another.
#[test]
fn a_subject_cannot_close_the_filter_and_open_another() {
    let config = LdapConfig::new("ldaps://ldap.example.com", "dc=example,dc=com");
    assert_eq!(
        config.filter_for("x)(uid=admin"),
        "(uid=x\\29\\28uid=admin)"
    );
}

/// The backslash has to be escaped first, or the escaping would escape
/// its own output.
#[test]
fn a_backslash_is_escaped_before_the_characters_it_would_introduce() {
    let config = LdapConfig::new("ldaps://x", "dc=x");
    assert_eq!(config.filter_for("a\\*b"), "(uid=a\\5c\\2ab)");
}

#[test]
fn a_custom_filter_takes_the_substitution_too() {
    let mut config = LdapConfig::new("ldaps://x", "dc=x");
    config.user_filter = "(&(objectClass=person)(mail={}))".to_string();
    assert_eq!(
        config.filter_for("ada@example.dev"),
        "(&(objectClass=person)(mail=ada@example.dev))"
    );
}

// ── memberOf values ────────────────────────────────────────────────────

#[test]
fn a_distinguished_name_yields_its_first_rdn() {
    assert_eq!(
        LdapConfig::group_name("CN=magistr-admins,OU=Groups,DC=example,DC=com"),
        "magistr-admins"
    );
}

#[test]
fn a_bare_group_name_passes_through_unchanged() {
    assert_eq!(LdapConfig::group_name("magistr-admins"), "magistr-admins");
}

/// A comma inside a CN is escaped in the DN; splitting on it naively
/// would cut the name in half.
#[test]
fn an_escaped_comma_does_not_end_the_first_rdn() {
    assert_eq!(
        LdapConfig::group_name("CN=Platform\\, Core,OU=Groups,DC=example,DC=com"),
        "Platform, Core"
    );
}

#[test]
fn whitespace_around_a_dn_component_is_dropped() {
    assert_eq!(
        LdapConfig::group_name("  CN = eng-platform , OU=Groups "),
        "eng-platform"
    );
}

// ── The port, and what a lookup resolves to ────────────────────────────

/// A directory that answers, and that can be told to stop answering.
struct FakeDirectory {
    entries: HashMap<String, Vec<String>>,
    reachable: RefCell<bool>,
    filters: RefCell<Vec<String>>,
    config: LdapConfig,
}

impl FakeDirectory {
    fn new() -> Self {
        let mut entries = HashMap::new();
        entries.insert(
            "ada".to_string(),
            vec![
                "CN=eng-platform,OU=Groups,DC=example,DC=com".to_string(),
                "CN=magistr-owners,OU=Groups,DC=example,DC=com".to_string(),
            ],
        );
        entries.insert(
            "grace".to_string(),
            vec!["CN=finance,OU=Groups".to_string()],
        );
        FakeDirectory {
            entries,
            reachable: RefCell::new(true),
            filters: RefCell::new(Vec::new()),
            config: LdapConfig::new("ldaps://ldap.example.com", "dc=example,dc=com"),
        }
    }
}

impl GroupSource for FakeDirectory {
    fn groups(&self, subject: &str) -> Result<Option<Vec<String>>, MagistrError> {
        self.filters
            .borrow_mut()
            .push(self.config.filter_for(subject));
        if !*self.reachable.borrow() {
            return Err(MagistrError::Other("connection refused".into()));
        }
        Ok(self
            .entries
            .get(subject)
            .map(|dns| dns.iter().map(|dn| LdapConfig::group_name(dn)).collect()))
    }
}

fn mapping() -> Vec<GroupRole> {
    vec![
        GroupRole {
            group: "eng-*".into(),
            role: TeamRole::Developer,
        },
        GroupRole {
            group: "magistr-owners".into(),
            role: TeamRole::Owner,
        },
    ]
}

#[test]
fn group_membership_resolves_to_a_role() {
    let directory = FakeDirectory::new();
    let outcome = DirectoryOutcome::from_lookup(directory.groups("ada"), &mapping());
    assert_eq!(outcome, DirectoryOutcome::Granted(TeamRole::Owner));
}

#[test]
fn a_member_of_no_mapped_group_gets_no_role() {
    let directory = FakeDirectory::new();
    let outcome = DirectoryOutcome::from_lookup(directory.groups("grace"), &mapping());
    assert_eq!(outcome, DirectoryOutcome::NoMatch);
}

#[test]
fn a_subject_the_directory_has_never_heard_of_gets_no_role() {
    let directory = FakeDirectory::new();
    let outcome = DirectoryOutcome::from_lookup(directory.groups("nobody"), &mapping());
    assert_eq!(outcome, DirectoryOutcome::NoMatch);
}

/// The acceptance criterion at the transport: a directory that stops
/// answering yields no role at all, never the role it answered with a
/// moment ago and never the fallback.
#[test]
fn a_directory_that_goes_unreachable_grants_nothing() {
    let directory = FakeDirectory::new();
    assert_eq!(
        DirectoryOutcome::from_lookup(directory.groups("ada"), &mapping()),
        DirectoryOutcome::Granted(TeamRole::Owner)
    );
    *directory.reachable.borrow_mut() = false;
    match DirectoryOutcome::from_lookup(directory.groups("ada"), &mapping()) {
        DirectoryOutcome::Unavailable(why) => assert!(why.contains("connection refused")),
        other => panic!("an unreachable directory must not answer, got {other:?}"),
    }
}

#[test]
fn every_lookup_goes_out_as_an_escaped_filter() {
    let directory = FakeDirectory::new();
    let _ = directory.groups("x)(uid=admin");
    assert_eq!(
        directory.filters.borrow().as_slice(),
        ["(uid=x\\29\\28uid=admin)".to_string()]
    );
}

#[test]
fn a_config_round_trips_through_json_with_its_defaults() {
    let config = LdapConfig::new("ldaps://ldap.example.com", "dc=example,dc=com");
    let text = serde_json::to_string(&config).expect("serialize");
    let back: LdapConfig = serde_json::from_str(&text).expect("deserialize");
    assert_eq!(back, config);

    let minimal: LdapConfig =
        serde_json::from_str(r#"{"url":"ldaps://x","base_dn":"dc=x"}"#).expect("defaults apply");
    assert_eq!(minimal.user_filter, "(uid={})");
    assert_eq!(minimal.group_attribute, "memberOf");
}
