use super::*;
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
        .ship_set_objective("Compose".into(), vec![], vec![], "chunk:test".into())
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
    assert_eq!(engines.ship_active_task_id().as_deref(), Some("verify"));
}

#[test]
fn a_check_with_no_task_is_refused_before_any_bytes_are_hashed() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (engines, root) = host(&tmp);
    engines
        .ship_set_objective("Compose".into(), vec![], vec![], "chunk:test".into())
        .expect("objective");
    let refused = AdmittedCheck::admit(&engines, &root, None, "lint", "true".into(), true)
        .expect_err("no active task");
    assert!(
        matches!(&refused, CheckEvidenceError::Admission(reason) if reason.contains("no active task")),
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
            assert_eq!(receipt.id, receipt_id);
            assert_eq!(receipt.target, original);
            assert!(matches!(
                receipt.settlement,
                Some(ReceiptSettlement::Stale { .. })
            ));
        }
        CheckSettlement::Applied { .. } => panic!("a superseded task must not collect the result"),
    }
    assert_eq!(settled.freshness(), Some(&SourceFreshness::Unchanged));
    assert!(
        engines.ship_status()["checks"]
            .as_array()
            .is_some_and(Vec::is_empty),
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
        assert_eq!(settled.receipt().execution, Some(execution));
    }
}

#[test]
fn an_unobservable_root_is_a_source_refusal_at_admission() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let absent = tmp.path().join("absent");
    let config = magistr_core::evidence::writer_config(&absent, |_| None)
        .with_data_dir(tmp.path().join("store"));
    let engines = ProjectAuthority::admit_for_project(&config, "absent".into(), WriterRole::Host)
        .expect("host admitted")
        .open();
    objective(&engines, "verify");
    let refused = AdmittedCheck::admit(&engines, &absent, None, "test", "true".into(), true)
        .expect_err("missing root");
    assert!(
        matches!(&refused, CheckEvidenceError::Source(reason) if reason.contains("does not exist")),
        "{refused}"
    );
    assert!(
        engines.ship_status()["checks"]
            .as_array()
            .is_some_and(Vec::is_empty),
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
        (key == "WORKBENCH_ORG_POLICY").then(|| named.clone())
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
