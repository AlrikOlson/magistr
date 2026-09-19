use super::*;
use crate::output::SilentRenderer;
use crate::team::TeamRole;

/// A capture with nothing set: no identity override, no CI flag, no home
/// — the roster is exercised without touching the process environment.
fn inputs(dir: &Path) -> ProjectInputs {
    ProjectInputs::from_lookup(dir, |_| None)
}

fn add_action(id: &str, role: &str) -> TeamAction {
    TeamAction::Add {
        id: id.to_string(),
        role: role.to_string(),
        name: None,
    }
}

#[test]
fn show_succeeds_in_solo_mode() {
    let tmp = tempfile::TempDir::new().unwrap();
    let renderer = SilentRenderer::new();
    cmd_team(tmp.path(), &inputs(tmp.path()), None, &renderer).unwrap();
    // Solo mode: showing must not create the roster file.
    assert!(!TeamConfig::path(tmp.path()).exists());
}

#[test]
fn init_creates_roster_with_one_owner() {
    let tmp = tempfile::TempDir::new().unwrap();
    let renderer = SilentRenderer::new();
    cmd_team(
        tmp.path(),
        &inputs(tmp.path()),
        Some(TeamAction::Init),
        &renderer,
    )
    .unwrap();

    let config = TeamConfig::load(tmp.path()).unwrap().unwrap();
    assert_eq!(config.members.len(), 1);
    assert_eq!(config.members[0].role, TeamRole::Owner);
}

#[test]
fn init_refuses_existing_roster() {
    let tmp = tempfile::TempDir::new().unwrap();
    let renderer = SilentRenderer::new();
    cmd_team(
        tmp.path(),
        &inputs(tmp.path()),
        Some(TeamAction::Init),
        &renderer,
    )
    .unwrap();

    let err = cmd_team(
        tmp.path(),
        &inputs(tmp.path()),
        Some(TeamAction::Init),
        &renderer,
    )
    .unwrap_err();
    assert!(err.to_string().contains("already exists"), "{err}");
}

#[test]
fn add_requires_roster() {
    let tmp = tempfile::TempDir::new().unwrap();
    let renderer = SilentRenderer::new();
    let err = cmd_team(
        tmp.path(),
        &inputs(tmp.path()),
        Some(add_action("dev@example.dev", "developer")),
        &renderer,
    )
    .unwrap_err();
    assert!(err.to_string().contains("magistr team init"), "{err}");
}

#[test]
fn add_remove_set_role_roundtrip() {
    let tmp = tempfile::TempDir::new().unwrap();
    let renderer = SilentRenderer::new();
    cmd_team(
        tmp.path(),
        &inputs(tmp.path()),
        Some(TeamAction::Init),
        &renderer,
    )
    .unwrap();
    cmd_team(
        tmp.path(),
        &inputs(tmp.path()),
        Some(add_action("dev@example.dev", "developer")),
        &renderer,
    )
    .unwrap();

    let config = TeamConfig::load(tmp.path()).unwrap().unwrap();
    assert_eq!(config.members.len(), 2);
    assert_eq!(config.members[1].id, "dev@example.dev");
    assert_eq!(config.members[1].role, TeamRole::Developer);

    cmd_team(
        tmp.path(),
        &inputs(tmp.path()),
        Some(TeamAction::SetRole {
            id: "dev@example.dev".to_string(),
            role: "admin".to_string(),
        }),
        &renderer,
    )
    .unwrap();
    let config = TeamConfig::load(tmp.path()).unwrap().unwrap();
    assert_eq!(config.members[1].role, TeamRole::Admin);

    cmd_team(
        tmp.path(),
        &inputs(tmp.path()),
        Some(TeamAction::Remove {
            id: "dev@example.dev".to_string(),
        }),
        &renderer,
    )
    .unwrap();
    let config = TeamConfig::load(tmp.path()).unwrap().unwrap();
    assert_eq!(config.members.len(), 1);
}

#[test]
fn add_rejects_unknown_role_naming_valid_ones() {
    let tmp = tempfile::TempDir::new().unwrap();
    let renderer = SilentRenderer::new();
    cmd_team(
        tmp.path(),
        &inputs(tmp.path()),
        Some(TeamAction::Init),
        &renderer,
    )
    .unwrap();

    let err = cmd_team(
        tmp.path(),
        &inputs(tmp.path()),
        Some(add_action("x@example.dev", "wizard")),
        &renderer,
    )
    .unwrap_err();
    assert!(
        err.to_string()
            .contains("owner, admin, developer, viewer, ci"),
        "{err}"
    );
}

#[test]
fn last_owner_guard_bubbles_up() {
    let tmp = tempfile::TempDir::new().unwrap();
    let renderer = SilentRenderer::new();
    cmd_team(
        tmp.path(),
        &inputs(tmp.path()),
        Some(TeamAction::Init),
        &renderer,
    )
    .unwrap();
    let owner_id = TeamConfig::load(tmp.path()).unwrap().unwrap().members[0]
        .id
        .clone();

    let err = cmd_team(
        tmp.path(),
        &inputs(tmp.path()),
        Some(TeamAction::Remove { id: owner_id }),
        &renderer,
    )
    .unwrap_err();
    assert!(err.to_string().contains("last Owner"), "{err}");
}
