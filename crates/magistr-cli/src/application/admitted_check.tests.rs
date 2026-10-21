use super::*;

#[tokio::test]
async fn a_host_gate_reuses_its_actual_execution_and_explicit_reruns_still_run() {
    let tmp = tempfile::tempdir().unwrap();
    let (engines, root) = host(&tmp);
    std::fs::create_dir_all(root.join(".magistr")).unwrap();
    let config = serde_json::json!({
        "project":{"name":"test","description":"","type":"cli"},
        "stack":{"language":"rust","framework":"none","package_manager":"cargo","test_framework":"cargo-test","linter":"clippy","formatter":"rustfmt"},
        "quality_gate":{"check":"printf checked","test":"true","lint":"true"},
        "format":{"extensions":["rs"],"command":"cargo fmt"},
        "testing":{"companion_pattern":"{name}.tests.{ext}","test_dir":"alongside"},
        "reference_repos":[],"state_prefix":"test",
        "verification":{"gates":{"check":{"reuse":true,"inputs":["notes.txt"],"resources":[]}}}
    });
    std::fs::write(root.join(".magistr/project.json"), config.to_string()).unwrap();
    objective(&engines, "verify");
    let admitted = AdmittedCheck::admit(
        &engines,
        &root,
        None,
        "check",
        "printf checked".into(),
        true,
    )
    .unwrap();
    assert!(admitted.reuse(&engines).unwrap().is_none());
    let run = crate::utils::gate::run_gate(
        "check",
        "printf checked",
        true,
        &root,
        5,
        &tokio_util::sync::CancellationToken::new(),
        |_| {},
    )
    .await;
    let original = admitted
        .settle(&engines, run.execution(), run.details())
        .unwrap();
    let second = AdmittedCheck::admit(
        &engines,
        &root,
        None,
        "check",
        "printf checked".into(),
        true,
    )
    .unwrap();
    let reused = second.reuse(&engines).unwrap().unwrap();
    assert!(
        matches!(reused.receipt().execution(), Some(CheckExecution::Reused { receipt_id }) if receipt_id == &original.receipt().id)
    );
    assert_eq!(reused.check().unwrap().exit_code, None);
    let forced = AdmittedCheck::admit_with_options(
        &engines,
        &root,
        None,
        "check",
        "printf checked".into(),
        true,
        magistr_engine::native::CheckOptions {
            reuse: false,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(forced.reuse(&engines).unwrap().is_none());
    std::fs::write(root.join("notes.txt"), "changed").unwrap();
    let changed = AdmittedCheck::admit(
        &engines,
        &root,
        None,
        "check",
        "printf checked".into(),
        true,
    )
    .unwrap();
    assert!(changed.reuse(&engines).unwrap().is_none());
}

use magistr_engine::native::{
    CheckSettlement, ProjectAuthority, ReceiptSettlement, SourceFreshness, TaskType, WriterRole,
};

/// A host writer admitted the way production admits one: on a writer
/// config carrying the project's own evidence capability.
fn host(tmp: &tempfile::TempDir) -> (NativeEngines, PathBuf) {
    let root = tmp.path().join("project");
    std::fs::create_dir_all(root.join(".git")).expect("project directory");
    std::fs::write(root.join("notes.txt"), "alpha\n").expect("source");
    let store = tmp.path().join("store");
    let config = magistr_core::evidence::writer_config(&root, |_| None).with_data_dir(store);
    let engines =
        ProjectAuthority::admit_for_project(&config, "composition".into(), WriterRole::Host)
            .expect("host admitted")
            .open();
    (engines, root)
}

fn objective(engines: &NativeEngines, task: &str) {
    engines
        .ship_set_objective_with_verification(
            "Compose".into(),
            vec![],
            vec![],
            "chunk:test".into(),
            magistr_engine::native::VerificationMode::Full,
        )
        .expect("objective");
    engines
        .ship_add_task(task.into(), "Verify".into(), TaskType::Test, None)
        .expect("task");
    engines.ship_start_task(task).expect("active task");
}

#[test]
fn admission_binds_the_task_and_the_source_before_the_run_and_settles_both() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (engines, root) = host(&tmp);
    objective(&engines, "verify");

    let admitted = AdmittedCheck::admit(&engines, &root, None, "test", "true".into(), true)
        .expect("admitted with source");
    let identities = admitted.identities();
    assert_eq!(identities.project_id, engines.project_id());
    assert_eq!(identities.task_id, "verify");
    assert!(!identities.task_instance.is_empty());
    assert!(!identities.run_id.is_empty());
    let workspace = identities.workspace.expect("source identity");
    assert_eq!(workspace.root, root.canonicalize().expect("canonical"));
    assert_eq!(
        workspace.generation, None,
        "no owner ever admitted this root"
    );
    assert_ne!(identities.run_id, workspace.root.to_string_lossy());
    let before = admitted.source().expect("observation").revision.clone();
    assert!(!admitted.receipt_id().is_empty());

    // An external edit while the "gate" runs: observed, never journaled.
    std::fs::write(root.join("notes.txt"), "beta\n").expect("external edit");

    let settled = admitted
        .settle(&engines, CheckExecution::Exited { code: 0 }, "ok".into())
        .expect("settled");
    assert!(settled.is_applied());
    let check = settled.check().expect("recorded check");
    assert!(check.passed && check.verified);
    assert_eq!(check.exit_code, Some(0));
    let receipt = settled.receipt();
    assert_eq!(receipt.source.as_ref().expect("source").revision, before);
    assert_eq!(receipt.settlement, Some(ReceiptSettlement::Applied));
    assert_eq!(
        settled.freshness(),
        Some(&SourceFreshness::Changed {
            paths: vec!["notes.txt (Modified)".into()]
        })
    );
    assert_eq!(
        engines.ship_active_task_id().unwrap().as_deref(),
        Some("verify")
    );
}

#[test]
fn a_check_with_no_task_is_refused_before_any_bytes_are_hashed() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (engines, root) = host(&tmp);
    engines
        .ship_set_objective_with_verification(
            "Compose".into(),
            vec![],
            vec![],
            "chunk:test".into(),
            magistr_engine::native::VerificationMode::Full,
        )
        .expect("objective");
    let refused = AdmittedCheck::admit(&engines, &root, None, "lint", "true".into(), true)
        .expect_err("no active task");
    assert!(
        matches!(&refused, CheckEvidenceError::Admission(reason) if reason.to_string().contains("no active task")),
        "{refused}"
    );
}

#[test]
fn a_superseded_objective_settles_stale_with_the_outcome_and_receipt_intact() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (engines, root) = host(&tmp);
    objective(&engines, "verify");
    let admitted = AdmittedCheck::admit(
        &engines,
        &root,
        Some("verify"),
        "check",
        "false".into(),
        true,
    )
    .expect("admitted");
    let original = admitted.target().clone();
    let receipt_id = admitted.receipt_id().to_owned();

    // The agent shipped and rolled the objective over while the gate ran;
    // the same task name in the new run is a different instance.
    engines.ship_finalize(vec![], None).expect("shipped");
    objective(&engines, "verify");

    let settled = admitted
        .settle(&engines, CheckExecution::Exited { code: 1 }, "red".into())
        .expect("settled, not an error");
    assert!(!settled.is_applied());
    match &settled.settlement {
        CheckSettlement::Stale {
            admission,
            outcome,
            receipt,
        } => {
            assert_eq!(admission.target, original);
            assert_eq!(outcome.exit_code, Some(1));
            assert!(!outcome.passed);
            assert!(outcome.verified);
            assert_eq!(receipt.value.id, receipt_id);
            assert_eq!(receipt.value.target, original);
            assert!(matches!(
                receipt.value.settlement,
                Some(ReceiptSettlement::Stale { .. })
            ));
        }
        CheckSettlement::Applied { .. } => panic!("a superseded task must not collect the result"),
    }
    assert_eq!(settled.freshness(), Some(&SourceFreshness::Unchanged));
    assert!(
        engines
            .ship_tasks()
            .unwrap()
            .iter()
            .all(|task| task.checks.is_empty()),
        "nothing lands on the new instance"
    );
}

#[test]
fn a_timeout_and_a_cancel_are_recorded_as_what_they_were() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (engines, root) = host(&tmp);
    objective(&engines, "verify");
    for execution in [CheckExecution::TimedOut, CheckExecution::Cancelled] {
        let admitted = AdmittedCheck::admit(&engines, &root, None, "test", "true".into(), true)
            .expect("admitted");
        let settled = admitted
            .settle(&engines, execution.clone(), String::new())
            .expect("settled");
        let check = settled.check().expect("recorded");
        assert!(!check.passed && check.verified);
        assert_eq!(check.exit_code, None);
        assert_eq!(settled.receipt().execution(), Some(&execution));
    }
}

#[test]
fn an_unobservable_root_is_a_source_refusal_at_admission() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let absent = tmp.path().join("absent");
    std::fs::create_dir(&absent).expect("root exists for host admission");
    let config = magistr_core::evidence::writer_config(&absent, |_| None)
        .with_data_dir(tmp.path().join("store"));
    let engines = ProjectAuthority::admit_for_project(&config, "absent".into(), WriterRole::Host)
        .expect("host admitted")
        .open();
    objective(&engines, "verify");
    std::fs::remove_dir_all(&absent).expect("root disappears after host admission");
    let refused = AdmittedCheck::admit(&engines, &absent, None, "test", "true".into(), true)
        .expect_err("missing root");
    assert!(
        matches!(&refused, CheckEvidenceError::Source(reason) if reason.to_string().contains("does not exist")),
        "{refused}"
    );
    assert!(
        engines
            .ship_tasks()
            .unwrap()
            .iter()
            .all(|task| task.checks.is_empty()),
        "nothing was recorded"
    );
}

use std::collections::BTreeMap;

use magistr_engine::native::CompletionRefusal;

/// A gate configuration whose standard gates and every named custom gate
/// run `true`.
fn gates_json(custom: &[&str]) -> String {
    serde_json::json!({
        "project": { "name": "p", "description": "", "type": "cli" },
        "stack": {
            "language": "rust", "framework": "none", "package_manager": "cargo",
            "test_framework": "cargo-test", "linter": "clippy", "formatter": "rustfmt"
        },
        "quality_gate": { "check": "true", "test": "true", "lint": "true" },
        "format": { "extensions": ["rs"], "command": "true" },
        "testing": { "companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside" },
        "reference_repos": [],
        "state_prefix": "p",
        "custom_gates": custom
            .iter()
            .map(|name| serde_json::json!({ "name": name, "command": "true", "required": true }))
            .collect::<Vec<_>>()
    })
    .to_string()
}

/// A host writer admitted the way production admits one, over inputs that
/// name an organization policy outside the tree.
fn governed_host(tmp: &tempfile::TempDir) -> (NativeEngines, PathBuf, PathBuf) {
    let root = tmp.path().join("project");
    std::fs::create_dir_all(root.join(".magistr")).expect("project directory");
    std::fs::write(root.join(".magistr/project.json"), gates_json(&[])).expect("gates");
    std::fs::write(root.join("notes.txt"), "alpha\n").expect("source");
    let policy = tmp.path().join("policy.toml");
    std::fs::write(&policy, "[security]\nredaction=true\n").expect("policy");
    let named = policy.display().to_string();
    let config = magistr_core::evidence::writer_config(&root, move |key| {
        (key == "MAGISTR_WORKSPACE_ORG_POLICY").then(|| named.clone())
    })
    .with_data_dir(tmp.path().join("store"));
    let engines =
        ProjectAuthority::admit_for_project(&config, "composition".into(), WriterRole::Host)
            .expect("host admitted")
            .open();
    (engines, root, policy)
}

fn passing_gate(engines: &NativeEngines, root: &Path, name: &str) -> SettledCheck {
    AdmittedCheck::admit(engines, root, None, name, "true".into(), true)
        .expect("admitted")
        .settle(engines, CheckExecution::Exited { code: 0 }, "ok".into())
        .expect("settled")
}

fn outstanding(refusal: &CompletionRefusal) -> BTreeMap<String, String> {
    let CompletionRefusal::Outstanding(evidence) = refusal else {
        panic!("{refusal}");
    };
    evidence
        .outstanding
        .iter()
        .map(|gate| (gate.gate.clone(), gate.reason.clone()))
        .collect()
}

#[test]
fn an_external_policy_or_the_command_scope_edited_around_a_gate_is_evidence_drift() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (engines, root, policy) = governed_host(&tmp);
    objective(&engines, "verify");
    for name in ["check", "test", "lint"] {
        let settled = passing_gate(&engines, &root, name);
        assert_eq!(settled.freshness(), Some(&SourceFreshness::Unchanged));
    }
    let evidence = engines
        .ship_verify_completion("chunk:test", Some("verify"))
        .expect("every required gate has current evidence");
    assert!(evidence.is_complete());
    assert!(!evidence.config_digest.is_empty());

    // The organization policy changes after the gates ran; no source byte
    // moves. The evidence no longer applies, and says why.
    std::fs::write(&policy, "[security]\nredaction=false\n").expect("policy edit");
    let refusal = engines
        .ship_verify_completion("chunk:test", Some("verify"))
        .expect_err("a changed external layer invalidates the evidence");
    let CompletionRefusal::Outstanding(later) = &refusal else {
        panic!("{refusal}");
    };
    assert_eq!(
        later.source_revision, evidence.source_revision,
        "no source byte moved"
    );
    assert_ne!(later.config_digest, evidence.config_digest);
    let reasons = outstanding(&refusal);
    assert_eq!(reasons.len(), 3);
    assert!(
        reasons.values().all(|reason| reason
            .contains("the applicable external configuration changed since the gate ran")),
        "{refusal}"
    );

    // A policy edited while a gate runs is named at settlement.
    let admitted = AdmittedCheck::admit(&engines, &root, None, "check", "true".into(), true)
        .expect("admitted");
    std::fs::write(&policy, "[security]\nredaction=true\n").expect("policy edit");
    let settled = admitted
        .settle(&engines, CheckExecution::Exited { code: 0 }, "ok".into())
        .expect("settled");
    assert_eq!(
        settled.freshness(),
        Some(&SourceFreshness::Reconfigured {
            layers: vec!["org-policy".into()]
        })
    );
    let refusal = engines
        .ship_verify_completion("chunk:test", Some("verify"))
        .expect_err("a reconfiguration during the run is not evidence");
    assert!(
        outstanding(&refusal)["check"]
            .contains("configuration changed while the gate ran: org-policy"),
        "{refusal}"
    );

    // Re-run under the current policy: the latest attempts govern.
    for name in ["check", "test", "lint"] {
        passing_gate(&engines, &root, name);
    }
    assert!(
        engines
            .ship_verify_completion("chunk:test", Some("verify"))
            .is_ok()
    );

    // The command scope widens after the gates: the gate configuration is a
    // source input, so the old receipts read as a source change, and the
    // new contract has no receipt at all.
    std::fs::write(
        root.join(".magistr/project.json"),
        gates_json(&["structure"]),
    )
    .expect("scope edit");
    let refusal = engines
        .ship_verify_completion("chunk:test", Some("verify"))
        .expect_err("a widened command scope invalidates the evidence");
    let reasons = outstanding(&refusal);
    assert!(reasons["structure"].contains("no receipt"), "{refusal}");
    assert!(
        reasons["check"].contains("the source changed since the gate ran"),
        "{refusal}"
    );
    for name in ["check", "test", "lint", "structure"] {
        passing_gate(&engines, &root, name);
    }
    assert!(
        engines
            .ship_verify_completion("chunk:test", Some("verify"))
            .is_ok()
    );
}
