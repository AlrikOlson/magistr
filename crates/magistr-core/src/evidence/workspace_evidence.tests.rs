use super::*;

use std::fs;

use magistr_engine::tas::{CheckType, SourceFreshness};

fn project(root: &Path) {
    fs::create_dir_all(root.join(".magistr")).unwrap();
    fs::write(
        root.join(".magistr/project.json"),
        serde_json::json!({
            "project": { "name": "p", "description": "", "type": "cli" },
            "stack": {
                "language": "rust", "framework": "none", "package_manager": "cargo",
                "test_framework": "cargo-test", "linter": "clippy", "formatter": "rustfmt"
            },
            "quality_gate": { "check": "cargo check", "test": "cargo test", "lint": "cargo clippy" },
            "format": { "extensions": ["rs"], "command": "cargo fmt" },
            "testing": { "companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside" },
            "reference_repos": [],
            "state_prefix": "p",
            "custom_gates": [
                { "name": "structure", "command": "./lint.sh", "required": true },
                { "name": "optional", "command": "./opt.sh", "required": false }
            ]
        })
        .to_string(),
    )
    .unwrap();
    fs::write(root.join("main.rs"), "fn main() {}\n").unwrap();
}

#[test]
fn observation_is_content_addressed_and_validation_names_the_changed_path() {
    let dir = tempfile::tempdir().unwrap();
    project(dir.path());
    let evidence = WorkspaceEvidence::new(dir.path());
    let witness = evidence.observe().unwrap();
    let before = witness.observation();
    assert_eq!(before.scheme, magistr_workspace::snapshot::SCHEME);
    assert_eq!(before.consistency, "optimistic");
    assert_eq!(witness.validate().unwrap(), SourceFreshness::Unchanged);

    fs::write(dir.path().join("main.rs"), "fn main() { panic!() }\n").unwrap();
    assert_eq!(
        witness.validate().unwrap(),
        SourceFreshness::Changed {
            paths: vec!["main.rs (Modified)".into()]
        }
    );
    let after = evidence.observe().unwrap().observation();
    assert_ne!(before.revision, after.revision);
    assert_eq!(before.policy_digest, after.policy_digest);
}

#[test]
fn a_gate_configuration_edit_moves_the_revision_and_the_contracts() {
    let dir = tempfile::tempdir().unwrap();
    project(dir.path());
    let evidence = WorkspaceEvidence::new(dir.path());
    let before = evidence.observe().unwrap().observation();
    let contracts = evidence.required_gates().unwrap();
    assert_eq!(
        contracts
            .iter()
            .map(|c| (c.name.as_str(), c.command.as_str(), c.check_type.clone()))
            .collect::<Vec<_>>(),
        vec![
            ("check", "cargo check", CheckType::Typecheck),
            ("test", "cargo test", CheckType::Test),
            ("lint", "cargo clippy", CheckType::Lint),
            ("structure", "./lint.sh", CheckType::Lint),
        ],
        "required custom gates in, optional ones out"
    );

    let path = dir.path().join(".magistr/project.json");
    let edited = fs::read_to_string(&path)
        .unwrap()
        .replace("cargo test", "cargo nextest run");
    fs::write(&path, edited).unwrap();
    let after = evidence.observe().unwrap().observation();
    assert_ne!(
        before.revision, after.revision,
        "project.json is a config input"
    );
    assert_eq!(
        evidence.required_gates().unwrap()[1].command,
        "cargo nextest run"
    );
}

#[test]
fn a_missing_root_or_configuration_refuses_in_words() {
    let dir = tempfile::tempdir().unwrap();
    let missing = WorkspaceEvidence::new(dir.path().join("nope"));
    let refused = match missing.observe() {
        Err(reason) => reason,
        Ok(_) => panic!("a missing root observes nothing"),
    };
    assert!(refused.contains("does not exist"), "{refused}");
    let bare = WorkspaceEvidence::new(dir.path());
    assert!(
        bare.required_gates()
            .unwrap_err()
            .contains("read gate configuration")
    );
}

#[test]
fn writer_config_carries_the_capability() {
    let dir = tempfile::tempdir().unwrap();
    project(dir.path());
    let config = super::super::writer_config(dir.path(), |_| None);
    assert!(config.evidence().is_some());
    assert_eq!(config.project_root(), dir.path());
}
