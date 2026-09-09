//! Team commands exercised through the CLI.
//!
//! Each step runs a separate `magistr` process with temporary home and data
//! directories. `MAGISTR_USER` selects the acting identity. Coverage includes
//! identity verification, role-based access, audit filtering and export,
//! session ownership and handoff, and `.magistr/team.json` configuration.

#[path = "support/magistr_command.rs"]
mod magistr_command;

use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

use magistr::session::session_record::SessionRecord;
use magistr::session::storage::{insert_session, open_db};

/// One magistr invocation whose whole world — data dir, home, identity,
/// working directory — is this test's.
///
/// The project directory is the working directory rather than a
/// positional: `magistr team`'s `dir` argument is only reachable when no
/// subcommand follows it, so every roster edit resolves its project the
/// way a real invocation does.
fn magistr(home: &TempDir, user: &str, project: &Path) -> Command {
    let mut cmd = Command::from_std(magistr_command::magistr_command());
    cmd.current_dir(project)
        .env("XDG_DATA_HOME", home.path().join("data"))
        .env("HOME", home.path())
        .env("MAGISTR_USER", user)
        // A CI runner would otherwise resolve unmatched identities to Ci.
        .env_remove("CI")
        .env_remove("GITHUB_ACTIONS");
    cmd
}

fn write_roster(project: &Path, json: &str) {
    let dir = project.join(".magistr");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("team.json"), json).unwrap();
}

fn read_roster(project: &Path) -> serde_json::Value {
    let text = std::fs::read_to_string(project.join(".magistr/team.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// The role `id` holds in the roster on disk.
fn role_of(project: &Path, id: &str) -> Option<String> {
    read_roster(project)["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == id)
        .map(|m| m["role"].as_str().unwrap().to_string())
}

const ALICE: &str = "Alice <alice@example.dev>";
const BOB: &str = "Bob <bob@example.dev>";
const MALLORY: &str = "Mallory <mallory@example.dev>";

/// Alice owns the team; Bob administers it; Mallory only watches.
fn seed_roster(project: &Path) {
    write_roster(
        project,
        r#"{
  "version": 1,
  "default_role": "viewer",
  "members": [
    { "id": "alice@example.dev", "name": "Alice", "role": "owner" },
    { "id": "bob@example.dev", "name": "Bob", "role": "admin" },
    { "id": "mallory@example.dev", "name": "Mallory", "role": "viewer" }
  ]
}
"#,
    );
}

// ── 1. Identity & team configuration ────────────────────────────────

#[test]
fn solo_mode_needs_no_roster_and_makes_you_owner() {
    let home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();

    magistr(&home, ALICE, project.path())
        .args(["--json", "team", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""solo": true"#))
        .stdout(predicate::str::contains(r#""role": "owner""#));

    assert!(!project.path().join(".magistr/team.json").exists());
}

#[test]
fn team_init_writes_the_roster_with_the_resolved_identity_as_owner() {
    let home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();

    magistr(&home, ALICE, project.path())
        .args(["team", "init"])
        .assert()
        .success();

    assert_eq!(
        role_of(project.path(), "alice@example.dev").as_deref(),
        Some("owner")
    );
}

#[test]
fn the_roster_decides_the_role_and_show_lists_the_capabilities() {
    let home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    seed_roster(project.path());

    for (user, role) in [
        (ALICE, "owner"),
        (BOB, "admin"),
        (MALLORY, "viewer"),
        // Not on the roster at all: default_role.
        ("Nobody <nobody@example.dev>", "viewer"),
    ] {
        magistr(&home, user, project.path())
            .args(["--json", "team", "show"])
            .assert()
            .success()
            .stdout(predicate::str::contains(format!(r#""role": "{role}""#)));
    }
}

// ── 2. Role-based access control ────────────────────────────────────

#[test]
fn an_owner_may_edit_the_roster() {
    let home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    seed_roster(project.path());

    magistr(&home, ALICE, project.path())
        .args(["team", "add", "dev@example.dev", "--role", "developer"])
        .assert()
        .success();

    assert_eq!(
        role_of(project.path(), "dev@example.dev").as_deref(),
        Some("developer")
    );
}

#[test]
fn a_viewer_may_not_edit_the_roster() {
    let home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    seed_roster(project.path());

    magistr(&home, MALLORY, project.path())
        .args(["team", "add", "dev@example.dev", "--role", "developer"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("may not edit the roster"));

    assert!(role_of(project.path(), "dev@example.dev").is_none());
}

#[test]
fn an_admin_may_not_promote_themself_to_owner() {
    let home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    seed_roster(project.path());

    magistr(&home, BOB, project.path())
        .args(["team", "set-role", "bob@example.dev", "owner"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("transfer ownership"));

    assert_eq!(
        role_of(project.path(), "bob@example.dev").as_deref(),
        Some("admin")
    );
}

#[test]
fn an_admin_may_not_demote_the_owner_but_may_promote_a_viewer() {
    let home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    seed_roster(project.path());

    magistr(&home, BOB, project.path())
        .args(["team", "set-role", "alice@example.dev", "viewer"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("grant or alter Owner"));

    magistr(&home, BOB, project.path())
        .args(["team", "set-role", "mallory@example.dev", "developer"])
        .assert()
        .success();

    assert_eq!(
        role_of(project.path(), "alice@example.dev").as_deref(),
        Some("owner")
    );
    assert_eq!(
        role_of(project.path(), "mallory@example.dev").as_deref(),
        Some("developer")
    );
}

#[test]
fn only_the_owner_can_hand_ownership_on() {
    let home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    seed_roster(project.path());

    magistr(&home, ALICE, project.path())
        .args(["team", "set-role", "bob@example.dev", "owner"])
        .assert()
        .success();

    assert_eq!(
        role_of(project.path(), "bob@example.dev").as_deref(),
        Some("owner")
    );
}

// ── 3. The audit log: attribution, filtering, export ────────────────

#[test]
fn roster_edits_reach_the_audit_log_with_their_actor() {
    let home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    seed_roster(project.path());

    // One allowed edit by the Owner…
    magistr(&home, ALICE, project.path())
        .args(["team", "add", "dev@example.dev", "--role", "developer"])
        .assert()
        .success();

    // …and one refusal by the Admin who reached for Owner.
    magistr(&home, BOB, project.path())
        .args(["team", "set-role", "bob@example.dev", "owner"])
        .assert()
        .failure();

    let out = magistr(&home, ALICE, project.path())
        .args(["permissions", "--export", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let entries: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let entries = entries.as_array().unwrap();

    assert_eq!(entries.len(), 2, "{entries:#?}");
    assert_eq!(entries[0]["tool_name"], "team_roster");
    assert_eq!(entries[0]["actor"], "alice@example.dev");
    assert_eq!(entries[0]["allowed"], true);
    assert_eq!(entries[1]["actor"], "bob@example.dev");
    assert_eq!(entries[1]["allowed"], false);
    assert_eq!(entries[1]["risk"], "high");

    // Filtering by actor narrows the view to one person's actions.
    magistr(&home, ALICE, project.path())
        .args(["--json", "permissions", "--actor", "bob@"])
        .assert()
        .success()
        .stdout(predicate::str::contains("bob@example.dev"))
        .stdout(predicate::str::contains("alice@example.dev").not());
}

#[test]
fn the_audit_log_exports_as_csv_with_an_actor_column() {
    let home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    seed_roster(project.path());

    magistr(&home, ALICE, project.path())
        .args(["team", "add", "dev@example.dev", "--role", "developer"])
        .assert()
        .success();

    magistr(&home, ALICE, project.path())
        .args(["permissions", "--export", "csv"])
        .assert()
        .success()
        .stdout(predicate::str::contains("actor"))
        .stdout(predicate::str::contains("alice@example.dev"))
        .stdout(predicate::str::contains("team_roster"));
}

// ── 4. Session ownership, sharing and handoff ───────────────────────

fn seed_session(home: &TempDir, id: &str, owner: &str) {
    let db = home.path().join("data/magistr/sessions.db");
    std::fs::create_dir_all(db.parent().unwrap()).unwrap();
    let conn = open_db(&db).unwrap();
    let now = chrono::Utc::now();
    insert_session(
        &conn,
        &SessionRecord {
            session_id: id.to_string(),
            project: "acme".to_string(),
            started_at: now - chrono::Duration::hours(1),
            ended_at: now,
            cost_usd: 0.5,
            turns: 2,
            duration_ms: 3_600_000,
            files_changed: vec![],
            tags: vec![],
            summary: None,
            model: String::new(),
            owner: Some(owner.to_string()),
        },
    )
    .unwrap();
}

#[test]
fn sessions_record_their_owner_and_filter_by_it() {
    let home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    seed_session(&home, "s-alice", "alice@example.dev");
    seed_session(&home, "s-bob", "bob@example.dev");

    magistr(&home, ALICE, project.path())
        .args(["sessions", "--format", "json", "--owner", "alice@"])
        .assert()
        .success()
        .stdout(predicate::str::contains("s-alice"))
        .stdout(predicate::str::contains("s-bob").not());
}

#[test]
fn a_viewer_may_not_take_a_session_over_but_an_admin_may() {
    let home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    seed_roster(project.path());
    seed_session(&home, "s-alice", "alice@example.dev");

    // `magistr sessions` resolves the roster from the working directory.
    magistr(&home, MALLORY, project.path())
        .args(["sessions", "--handoff", "s-alice", "mallory@example.dev"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("may not hand off"));

    magistr(&home, BOB, project.path())
        .args(["sessions", "--handoff", "s-alice", "bob@example.dev"])
        .assert()
        .success();

    magistr(&home, BOB, project.path())
        .args(["sessions", "--format", "json", "--owner", "bob@"])
        .assert()
        .success()
        .stdout(predicate::str::contains("s-alice"));
}

// ── 5. Verified identity gates the roster when the team asks ────────

#[test]
fn a_provider_requiring_roster_refuses_a_self_asserted_edit() {
    let home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    write_roster(
        project.path(),
        r#"{
  "version": 1,
  "default_role": "viewer",
  "identity_provider": { "kind": "github", "client_id": "abc123" },
  "members": [
    { "id": "alice@example.dev", "name": "Alice", "role": "owner" }
  ]
}
"#,
    );

    // Alice is the Owner and would otherwise be allowed — but nothing
    // has vouched for the identity `MAGISTR_USER` asserted.
    magistr(&home, ALICE, project.path())
        .args(["team", "add", "dev@example.dev", "--role", "developer"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("magistr login"));

    assert!(role_of(project.path(), "dev@example.dev").is_none());
}

#[test]
fn login_status_reports_an_empty_cache_without_contacting_anyone() {
    let home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();

    magistr(&home, ALICE, project.path())
        .args(["login", "--status"])
        .assert()
        .success();
}
