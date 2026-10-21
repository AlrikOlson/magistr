use super::*;

use std::fs;

use magistr_engine::native::{CheckType, SourceFreshness};

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

/// A project beside the external layers a host's environment would name:
/// an organization policy and a user layer outside the tree.
fn rig() -> (tempfile::TempDir, PathBuf, WorkspaceEvidence) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    fs::create_dir_all(&root).unwrap();
    project(&root);
    let external = dir.path().join("external");
    fs::create_dir_all(&external).unwrap();
    fs::write(external.join("policy.toml"), "[security]\nredaction=true\n").unwrap();
    let org = external.join("policy.toml").display().to_string();
    let user = external.join("config.toml").display().to_string();
    let inputs = ProjectInputs::from_lookup(&root, move |key| match key {
        "MAGISTR_WORKSPACE_ORG_POLICY" => Some(org.clone()),
        "MAGISTR_WORKSPACE_USER_CONFIG" => Some(user.clone()),
        _ => None,
    });
    let evidence = WorkspaceEvidence::from_inputs(&inputs);
    (dir, root, evidence)
}

#[test]
fn receipt_retention_refuses_unreadable_or_malformed_checkpoints() {
    let (_dir, root, evidence) = rig();
    assert!(evidence.retained_receipts().unwrap().is_empty());
    let path = root.join(".magistr/loop-state.json");
    fs::write(&path, b"{broken").unwrap();
    assert!(evidence.retained_receipts().is_err());
    fs::write(&path, b"{}").unwrap();
    assert!(evidence.retained_receipts().is_err());
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(evidence.retained_receipts().is_err());
}

#[test]
fn observation_is_content_addressed_and_validation_names_the_changed_path() {
    let (_dir, root, evidence) = rig();
    let witness = evidence.observe().unwrap();
    let before = witness.observation();
    assert_eq!(before.scheme, magistr_workspace::snapshot::SCHEME);
    assert_eq!(before.consistency, "optimistic");
    assert!(!before.source_digest.is_empty() && !before.config_digest.is_empty());
    assert_ne!(before.source_digest, before.revision);
    assert_eq!(witness.validate().unwrap(), SourceFreshness::Unchanged);

    fs::write(root.join("main.rs"), "fn main() { panic!() }\n").unwrap();
    assert_eq!(
        witness.validate().unwrap(),
        SourceFreshness::Changed {
            paths: vec!["main.rs (Modified)".into()]
        }
    );
    let after = evidence.observe().unwrap().observation();
    assert_ne!(before.revision, after.revision);
    assert_ne!(before.source_digest, after.source_digest);
    assert_eq!(before.policy_digest, after.policy_digest);
    assert_eq!(before.config_digest, after.config_digest);
    assert_eq!(
        before.drift_to(&after),
        Some(magistr_engine::native::ObservationDrift::Source {
            before: before.source_digest.clone(),
            after: after.source_digest.clone(),
        })
    );
}

#[test]
fn an_external_layer_edit_moves_the_configuration_with_every_source_byte_unchanged() {
    let (dir, _root, evidence) = rig();
    let witness = evidence.observe().unwrap();
    let before = witness.observation();
    assert_eq!(
        evidence
            .layers()
            .iter()
            .map(|(layer, _)| *layer)
            .collect::<Vec<_>>(),
        vec![ConfigLayer::OrgPolicy, ConfigLayer::User]
    );

    // The user layer appears while the gate runs; the tree is untouched.
    fs::write(
        dir.path().join("external/config.toml"),
        "[telemetry]\nconsent='off'\n",
    )
    .unwrap();
    assert_eq!(
        witness.validate().unwrap(),
        SourceFreshness::Reconfigured {
            layers: vec!["user".into()]
        }
    );
    let after = evidence.observe().unwrap().observation();
    assert_eq!(before.revision, after.revision, "no source byte moved");
    assert_eq!(before.source_digest, after.source_digest);
    assert_eq!(before.policy_digest, after.policy_digest);
    assert_ne!(before.config_digest, after.config_digest);
    assert_eq!(
        before.drift_to(&after),
        Some(magistr_engine::native::ObservationDrift::Configuration {
            before: before.config_digest.clone(),
            after: after.config_digest.clone(),
        })
    );

    // Nothing a layer says, and nowhere it lives, is serialized.
    let recorded = serde_json::to_string(&after).unwrap();
    assert!(!recorded.contains("redaction") && !recorded.contains("consent"));
    assert!(!recorded.contains("policy.toml") && !recorded.contains("external"));

    // A layer that exists but cannot be read refuses the observation.
    fs::remove_file(dir.path().join("external/policy.toml")).unwrap();
    fs::create_dir_all(dir.path().join("external/policy.toml")).unwrap();
    let refused = match evidence.observe() {
        Err(reason) => reason,
        Ok(_) => panic!("an unreadable layer observes nothing"),
    };
    assert!(
        refused.contains("org-policy configuration layer"),
        "{refused}"
    );
}

#[test]
fn a_gate_configuration_edit_moves_the_revision_and_the_contracts() {
    let (_dir, root, evidence) = rig();
    let before = evidence.observe().unwrap().observation();
    let contracts = evidence.required_gates().unwrap();
    assert_eq!(
        contracts
            .iter()
            .map(|c| (c.name.as_str(), c.command.as_str(), c.check_type.clone()))
            .collect::<Vec<_>>(),
        vec![
            ("lint", "cargo clippy", CheckType::Lint),
            ("test", "cargo test", CheckType::Test),
            ("structure", "./lint.sh", CheckType::Lint),
        ],
        "required custom gates in, optional ones out"
    );
    assert_eq!(
        WorkspaceEvidence::gate_contracts(&root).unwrap(),
        contracts,
        "the port and the shared resolver are one"
    );

    let path = root.join(".magistr/project.json");
    let edited = fs::read_to_string(&path)
        .unwrap()
        .replace("cargo test", "cargo nextest run");
    fs::write(&path, edited).unwrap();
    let after = evidence.observe().unwrap().observation();
    assert_ne!(
        before.revision, after.revision,
        "project.json is a config input"
    );
    assert_ne!(before.source_digest, after.source_digest);
    assert_ne!(
        before.config_digest, after.config_digest,
        "the project settings are also shared configuration"
    );
    assert_eq!(
        evidence.required_gates().unwrap()[1].command,
        "cargo nextest run"
    );
}

#[test]
fn a_missing_root_or_configuration_refuses_in_words() {
    let dir = tempfile::tempdir().unwrap();
    let resolve = ResolveOptions::new(dir.path());
    let missing = WorkspaceEvidence::new(dir.path().join("nope"), &resolve);
    let refused = match missing.observe() {
        Err(reason) => reason,
        Ok(_) => panic!("a missing root observes nothing"),
    };
    assert!(refused.contains("does not exist"), "{refused}");
    let bare = WorkspaceEvidence::new(dir.path(), &resolve);
    assert!(
        bare.required_gates()
            .unwrap_err()
            .contains("read gate configuration")
    );
}

#[test]
fn writer_config_carries_the_capability_over_the_lookups_layers() {
    let dir = tempfile::tempdir().unwrap();
    project(dir.path());
    let config = super::super::writer_config(dir.path(), |_| None);
    assert!(config.evidence().is_some());
    assert_eq!(config.project_root(), dir.path());

    // The same lookup names the external layers the evidence digests.
    let policy = dir.path().join("org-policy.toml");
    fs::write(&policy, "[security]\nredaction=true\n").unwrap();
    let named = policy.display().to_string();
    let with_policy = super::super::writer_config(dir.path(), move |key| {
        (key == "MAGISTR_WORKSPACE_ORG_POLICY").then(|| named.clone())
    });
    let plain = config.evidence().unwrap().observe().unwrap().observation();
    let governed = with_policy
        .evidence()
        .unwrap()
        .observe()
        .unwrap()
        .observation();
    assert_eq!(plain.source_digest, governed.source_digest);
    assert_ne!(plain.config_digest, governed.config_digest);
}

#[test]
fn linked_copy_reads_and_writes_shared_settings_and_observes_their_changes() {
    let temp = tempfile::tempdir().unwrap();
    let main = temp.path().join("main");
    let linked = temp.path().join("task");
    let metadata = main.join(".git/worktrees/task");
    fs::create_dir_all(&metadata).unwrap();
    fs::create_dir_all(&linked).unwrap();
    project(&main);
    fs::write(linked.join("main.rs"), "fn main() {}\n").unwrap();
    fs::write(
        linked.join(".git"),
        format!("gitdir: {}", metadata.display()),
    )
    .unwrap();
    fs::write(metadata.join("commondir"), "../..").unwrap();
    fs::write(
        metadata.join("gitdir"),
        linked.join(".git").display().to_string(),
    )
    .unwrap();

    let inputs = ProjectInputs::from_lookup(&linked, |_| None);
    let path = inputs.project_json().unwrap();
    assert_eq!(
        path,
        main.canonicalize().unwrap().join(".magistr/project.json")
    );
    assert_eq!(
        inputs.project_config().unwrap().value.unwrap().project.name,
        "p"
    );
    assert_eq!(
        WorkspaceEvidence::gate_contracts(&linked).unwrap(),
        WorkspaceEvidence::gate_contracts(&main).unwrap()
    );
    let evidence = WorkspaceEvidence::from_inputs(&inputs);
    let witness = evidence.observe().unwrap();
    let before = witness.observation();
    let content = fs::read_to_string(&path)
        .unwrap()
        .replace("cargo test", "cargo nextest run");
    fs::write(&path, content).unwrap();
    assert_eq!(
        witness.validate().unwrap(),
        SourceFreshness::Reconfigured {
            layers: vec!["workspace".into()]
        }
    );
    let after = evidence.observe().unwrap().observation();
    assert_eq!(before.source_digest, after.source_digest);
    assert_ne!(before.config_digest, after.config_digest);
    assert_eq!(
        evidence.required_gates().unwrap()[1].command,
        "cargo nextest run"
    );
    assert!(!linked.join(".magistr/project.json").exists());
}

#[test]
fn a_check_scope_ignores_unrelated_edits_but_tracks_new_inputs_and_toolchain_settings() {
    let (_dir, root, evidence) = rig();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "pub fn run() {}").unwrap();
    let witness = evidence
        .observe_check("focused", Some("true"), &["src".into()])
        .unwrap();
    assert_eq!(witness.observation().inputs, ["src"]);
    assert!(!witness.observation().execution_digest.is_empty());
    fs::write(root.join("README.md"), "Unrelated copy").unwrap();
    assert_eq!(witness.validate().unwrap(), SourceFreshness::Unchanged);
    fs::write(root.join("src/new.rs"), "new input").unwrap();
    assert!(matches!(
        witness.validate().unwrap(),
        SourceFreshness::Changed { .. }
    ));
    let witness = evidence
        .observe_check("focused", Some("true"), &["src".into()])
        .unwrap();
    fs::create_dir(root.join(".cargo")).unwrap();
    fs::write(root.join(".cargo/config.toml"), "[build]\njobs = 2\n").unwrap();
    assert!(matches!(
        witness.validate().unwrap(),
        SourceFreshness::Reconfigured { .. }
    ));
}

#[test]
fn deadline_precedence_and_origin_are_shared_by_live_and_native_checks() {
    use magistr_engine::native::TimeoutOrigin;
    let (_dir, root, evidence) = rig();
    let path = root.join(".magistr/project.json");
    let mut config: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    for (policy, per_call, seconds, origin) in [
        (serde_json::json!({}), None, 900, TimeoutOrigin::Default),
        (
            serde_json::json!({"timeout_secs": 75}),
            None,
            75,
            TimeoutOrigin::Project,
        ),
        (
            serde_json::json!({"timeout_secs": 75, "gates": {"structure": {"timeout_secs": 25}}}),
            None,
            25,
            TimeoutOrigin::Gate,
        ),
        (
            serde_json::json!({"timeout_secs": 75, "gates": {"structure": {"timeout_secs": 25}}}),
            Some(5),
            5,
            TimeoutOrigin::PerCall,
        ),
    ] {
        config["verification"] = policy;
        fs::write(&path, config.to_string()).unwrap();
        let budget = evidence
            .check_budget("structure", Some("./lint.sh"), per_call)
            .unwrap();
        assert_eq!((budget.seconds, budget.origin), (seconds, origin));
        assert_eq!(
            crate::gates::execution_policy(&root, "structure", "./lint.sh", per_call)
                .unwrap()
                .0,
            seconds
        );
    }
    for invalid in [0, 3601] {
        assert!(
            evidence
                .check_budget("structure", Some("./lint.sh"), Some(invalid))
                .is_err()
        );
        assert!(
            crate::gates::execution_policy(&root, "structure", "./lint.sh", Some(invalid)).is_err()
        );
    }
}

#[test]
fn configured_gate_scopes_deadlines_and_reuse_require_the_configured_command() {
    let (_dir, root, evidence) = rig();
    let path = root.join(".magistr/project.json");
    let mut config: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    config["verification"] = serde_json::json!({
        "timeout_secs": 75,
        "gates": {"structure": {"inputs": ["main.rs"], "reuse": true, "timeout_secs": 25}}
    });
    fs::write(&path, config.to_string()).unwrap();
    assert_eq!(
        evidence
            .check_budget("structure", Some("./lint.sh"), None)
            .unwrap()
            .seconds,
        25
    );
    assert_eq!(
        evidence
            .check_budget("structure", Some("different"), None)
            .unwrap()
            .seconds,
        75
    );
    let scoped = evidence
        .observe_check("structure", Some("./lint.sh"), &[])
        .unwrap()
        .observation();
    assert_eq!(scoped.inputs, ["main.rs"]);
    let broad = evidence
        .observe_check("structure", Some("different"), &[])
        .unwrap()
        .observation();
    assert!(broad.inputs.is_empty());
}
