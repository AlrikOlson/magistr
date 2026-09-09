use super::*;

use magistr_engine::policy::GovernancePolicy;

use crate::team::directory::{DirectoryConfig, GroupRole};
use crate::team::{TeamMember, TeamRole};

fn identity(id: &str) -> Identity {
    Identity {
        name: id.to_string(),
        email: Some(id.to_string()),
        provider: None,
    }
}

fn roster(members: Vec<TeamMember>, directory: DirectoryConfig) -> TeamConfig {
    TeamConfig {
        version: 1,
        default_role: TeamRole::Viewer,
        identity_provider: None,
        budgets: Vec::new(),
        policy: GovernancePolicy::default(),
        directory,
        members,
    }
}

fn member(id: &str, role: TeamRole, active: bool) -> TeamMember {
    TeamMember {
        id: id.to_string(),
        name: None,
        role,
        active,
    }
}

fn directory() -> DirectoryConfig {
    DirectoryConfig::with_groups(vec![GroupRole {
        group: "eng-*".into(),
        role: TeamRole::Developer,
    }])
}

#[test]
fn solo_mode_is_still_owner_whatever_the_outcome_says() {
    let role = resolve_role_with_directory(
        None,
        &identity("solo@example.dev"),
        false,
        &DirectoryOutcome::Unavailable("down".into()),
    )
    .expect("solo mode never consults a directory");
    assert_eq!(role, TeamRole::Owner);
}

/// A team that has not opted in is untouched by every rule in this
/// module, including the refusal.
#[test]
fn a_team_with_no_directory_ignores_the_outcome_entirely() {
    let config = roster(Vec::new(), DirectoryConfig::default());
    let role = resolve_role_with_directory(
        Some(&config),
        &identity("nobody@example.dev"),
        false,
        &DirectoryOutcome::Unavailable("down".into()),
    )
    .expect("an un-configured team cannot fail closed");
    assert_eq!(role, TeamRole::Viewer);
}

#[test]
fn a_rostered_member_answers_from_the_roster() {
    let config = roster(
        vec![member("ada@example.dev", TeamRole::Admin, true)],
        directory(),
    );
    let role = resolve_role_with_directory(
        Some(&config),
        &identity("ada@example.dev"),
        false,
        &DirectoryOutcome::NoMatch,
    )
    .expect("a roster row is an explicit grant");
    assert_eq!(role, TeamRole::Admin);
}

/// The roster is the directory's own projection — SCIM writes it — so a
/// row already recorded needs no live lookup, and an outage cannot take
/// a provisioned member's access away.
#[test]
fn a_rostered_member_survives_the_directory_being_down() {
    let config = roster(
        vec![member("ada@example.dev", TeamRole::Admin, true)],
        directory(),
    );
    let role = resolve_role_with_directory(
        Some(&config),
        &identity("ada@example.dev"),
        false,
        &DirectoryOutcome::Unavailable("connection refused".into()),
    )
    .expect("an explicit row is not an implicit grant");
    assert_eq!(role, TeamRole::Admin);
}

#[test]
fn a_deprovisioned_member_is_refused_rather_than_falling_back() {
    let config = roster(
        vec![member("gone@example.dev", TeamRole::Developer, false)],
        directory(),
    );
    let error = resolve_role_with_directory(
        Some(&config),
        &identity("gone@example.dev"),
        false,
        &DirectoryOutcome::NoMatch,
    )
    .expect_err("a soft-deleted member must not become the default role");
    assert!(error.to_string().contains("deprovisioned"), "{error}");
}

#[test]
fn an_off_roster_identity_takes_the_role_the_directory_granted() {
    let config = roster(Vec::new(), directory());
    let role = resolve_role_with_directory(
        Some(&config),
        &identity("new@example.dev"),
        false,
        &DirectoryOutcome::Granted(TeamRole::Developer),
    )
    .expect("a granted role is a role");
    assert_eq!(role, TeamRole::Developer);
}

#[test]
fn an_answered_no_falls_through_to_the_rosters_own_default() {
    let config = roster(Vec::new(), directory());
    let role = resolve_role_with_directory(
        Some(&config),
        &identity("new@example.dev"),
        false,
        &DirectoryOutcome::NoMatch,
    )
    .expect("an answered no is data");
    assert_eq!(role, TeamRole::Viewer);
}

/// The acceptance criterion, stated as a test: an unreachable directory
/// grants nothing.
#[test]
fn an_unreachable_directory_grants_nothing() {
    let config = roster(Vec::new(), directory());
    let error = resolve_role_with_directory(
        Some(&config),
        &identity("new@example.dev"),
        false,
        &DirectoryOutcome::Unavailable("connection refused".into()),
    )
    .expect_err("a directory that did not answer must not grant a role");
    let message = error.to_string();
    assert!(message.contains("could not be reached"), "{message}");
    assert!(message.contains("connection refused"), "{message}");
}

/// Fail closed that exempts automation is not fail closed: CI is the
/// identity most likely to be running while a directory is down.
#[test]
fn the_ci_shortcut_does_not_survive_an_unreachable_directory() {
    let config = roster(Vec::new(), directory());
    let error = resolve_role_with_directory(
        Some(&config),
        &identity("runner@ci"),
        true,
        &DirectoryOutcome::Unavailable("timed out".into()),
    )
    .expect_err("CI must not walk past a directory outage");
    assert!(error.to_string().contains("could not be reached"));
}

#[test]
fn the_ci_shortcut_still_applies_when_the_directory_answered() {
    let config = roster(Vec::new(), directory());
    let role = resolve_role_with_directory(
        Some(&config),
        &identity("runner@ci"),
        true,
        &DirectoryOutcome::NoMatch,
    )
    .expect("an answered no keeps the behaviour a directory-less team had");
    assert_eq!(role, TeamRole::Ci);
}
