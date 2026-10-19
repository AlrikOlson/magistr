//! Real commands read the same current settings from linked working copies.

#[path = "support/magistr_command.rs"]
mod magistr_command;

use std::path::Path;
use std::process::Command;

use assert_cmd::Command as AssertCommand;
use predicates::prelude::*;
use tempfile::TempDir;

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn magistr(data: &Path) -> AssertCommand {
    let mut command = AssertCommand::from_std(magistr_command::magistr_command());
    command.env("MAGISTR_WORKFLOW_DATA_DIR", data);
    command
}

#[test]
fn linked_commands_and_setup_use_current_shared_settings_and_refuse_invalid_values() {
    let tmp = TempDir::new().unwrap();
    let main = tmp.path().join("main");
    let linked = tmp.path().join("linked");
    let data = tmp.path().join("workflow-data");
    std::fs::create_dir_all(&main).unwrap();
    git(&main, &["init", "--initial-branch=main"]);
    std::fs::write(main.join("README.md"), "fixture\n").unwrap();
    git(&main, &["add", "README.md"]);
    git(
        &main,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-m",
            "fixture",
        ],
    );
    git(
        &main,
        &["worktree", "add", "--detach", linked.to_str().unwrap()],
    );

    let mut config = serde_json::json!({
        "project": {"name": "settings-test", "type": "cli"},
        "stack": {"language": "unknown", "framework": "none", "package_manager": "unknown", "test_framework": "none", "linter": "none", "formatter": "none"},
        "quality_gate": {"check": "true", "test": "true", "lint": "true"},
        "format": {"command": "", "extensions": []},
        "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
        "reference_repos": [], "state_prefix": "settings-test", "roadmap_exports": false
    });
    config["project"]["name"] = "shared-project".into();
    config["quality_gate"]["check"] = "echo shared-check".into();
    let original = config.to_string();
    std::fs::create_dir_all(main.join(".magistr")).unwrap();
    let shared = main.join(".magistr/project.json");
    std::fs::write(&shared, &original).unwrap();
    std::fs::create_dir_all(linked.join(".magistr")).unwrap();
    std::fs::write(
        linked.join(".magistr/project.json"),
        "{invalid local settings",
    )
    .unwrap();
    std::fs::write(
        linked.join(".magistr-roadmap.yml"),
        "invalid unrelated YAML",
    )
    .unwrap();
    std::fs::write(
        linked.join("Cargo.toml"),
        "[package]\nname = \"detected\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let linked_path = linked.to_str().unwrap();

    magistr(&data)
        .args(["config", "show", linked_path])
        .assert()
        .success()
        .stderr(
            predicate::str::contains("shared-project")
                .and(predicate::str::contains("shared-check"))
                .and(predicate::str::contains("unknown")),
        );
    magistr(&data)
        .args(["config", "check", linked_path])
        .assert()
        .success();
    magistr(&data)
        .args(["roadmap", "status", linked_path, "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"counts\""));
    assert_eq!(std::fs::read_to_string(&shared).unwrap(), original);

    config["roadmap_backend"] = "invalid".into();
    let invalid = config.to_string();
    std::fs::write(&shared, &invalid).unwrap();
    for args in [
        vec!["config", "show", linked_path],
        vec!["config", "check", linked_path],
        vec!["roadmap", "status", linked_path],
        vec!["init", linked_path, "--force"],
    ] {
        magistr(&data)
            .args(args)
            .assert()
            .failure()
            .stderr(predicate::str::contains("Invalid project settings"));
    }
    assert_eq!(std::fs::read_to_string(&shared).unwrap(), invalid);
    assert!(!linked.join(".claude").exists());
    assert!(!linked.join(".mcp.json").exists());
    assert!(!linked.join(".magistr/workspace.toml").exists());
}
