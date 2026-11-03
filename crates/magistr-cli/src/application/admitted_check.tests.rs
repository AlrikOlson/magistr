use super::*;

#[tokio::test]
async fn a_host_check_reuses_its_actual_execution_and_explicit_reruns_still_run() {
    let tmp = tempfile::tempdir().unwrap();
    let (engines, root) = host(&tmp);
    std::fs::create_dir_all(root.join(".magistr")).unwrap();
    let config = serde_json::json!({
        "project":{"name":"test","description":"","type":"cli"},
        "stack":{"language":"rust","framework":"none","package_manager":"cargo","test_framework":"cargo-test","linter":"clippy","formatter":"rustfmt"},
        "checks":{"check":"printf checked","test":"true","lint":"true"},
        "format":{"extensions":["rs"],"command":"cargo fmt"},
        "testing":{"companion_pattern":"{name}.tests.{ext}","test_dir":"alongside"},
        "reference_repos":[],"state_prefix":"test",
        "verification":{"checks":{"check":{"reuse":true,"inputs":["notes.txt"],"resources":[]}}}
    });
    std::fs::write(root.join(".magistr/project.json"), config.to_string()).unwrap();
    goal(&engines, "verify");
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
    let run = crate::utils::check::run_check(
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
        matches!(reused.check_result().execution(), Some(CheckExecution::Reused { check_result_id }) if check_result_id == &original.check_result().id)
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
    CheckResultSettlement, CheckSettlement, ProjectAuthority, SourceFreshness, StepType, WriterRole,
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

fn goal(engines: &NativeEngines, step: &str) {
    engines
        .run_open_with_verification(
            "Compose".into(),
            vec![],
            vec![],
            "task:test".into(),
            magistr_engine::native::VerificationMode::Full,
        )
        .expect("goal");
    engines
        .add_step(step.into(), "Verify".into(), StepType::Test, None)
        .expect("step");
    engines.start_step(step).expect("active step");
}

#[test]
fn admission_binds_the_step_and_the_source_before_the_run_and_settles_both() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (engines, root) = host(&tmp);
    goal(&engines, "verify");

    let admitted = AdmittedCheck::admit(&engines, &root, None, "test", "true".into(), true)
        .expect("admitted with source");
    let identities = admitted.identities();
    assert_eq!(identities.project_id, engines.project_id());
    assert_eq!(identities.step_id, "verify");
    assert!(!identities.step_instance.is_empty());
    assert!(!identities.run_id.is_empty());
    let workspace = identities.workspace.expect("source identity");
    assert_eq!(workspace.root, root.canonicalize().expect("canonical"));
    assert_eq!(
        workspace.generation, None,
        "no owner ever admitted this root"
    );
    assert_ne!(identities.run_id, workspace.root.to_string_lossy());
    let before = admitted.source().expect("observation").revision.clone();
    assert!(!admitted.check_result_id().is_empty());

    // An external edit while the "check" runs: observed, never journaled.
    std::fs::write(root.join("notes.txt"), "beta\n").expect("external edit");

    let settled = admitted
        .settle(&engines, CheckExecution::Exited { code: 0 }, "ok".into())
        .expect("settled");
    assert!(settled.is_applied());
    let check = settled.check().expect("recorded check");
    assert!(check.passed && check.verified);
    assert_eq!(check.exit_code, Some(0));
    let check_result = settled.check_result();
    assert_eq!(
        check_result.source.as_ref().expect("source").revision,
        before
    );
    assert_eq!(
        check_result.settlement,
        Some(CheckResultSettlement::Applied)
    );
    assert_eq!(
        settled.freshness(),
        Some(&SourceFreshness::Changed {
            paths: vec!["notes.txt (Modified)".into()]
        })
    );
    assert_eq!(
        engines.run_active_step_id().unwrap().as_deref(),
        Some("verify")
    );
}

#[test]
fn a_check_with_no_step_is_refused_before_any_bytes_are_hashed() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (engines, root) = host(&tmp);
    engines
        .run_open_with_verification(
            "Compose".into(),
            vec![],
            vec![],
            "task:test".into(),
            magistr_engine::native::VerificationMode::Full,
        )
        .expect("goal");
    let refused = AdmittedCheck::admit(&engines, &root, None, "lint", "true".into(), true)
        .expect_err("no active step");
    assert!(
        matches!(&refused, CheckEvidenceError::Admission(reason) if reason.to_string().contains("no active step")),
        "{refused}"
    );
}

#[test]
fn a_superseded_goal_settles_stale_with_the_outcome_and_check_result_intact() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (engines, root) = host(&tmp);
    goal(&engines, "verify");
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
    let check_result_id = admitted.check_result_id().to_owned();

    // The agent shipped and rolled the goal over while the check ran;
    // the same step name in the new run is a different instance.
    engines.run_finish(vec![], None).expect("shipped");
    goal(&engines, "verify");

    let settled = admitted
        .settle(&engines, CheckExecution::Exited { code: 1 }, "red".into())
        .expect("settled, not an error");
    assert!(!settled.is_applied());
    match &settled.settlement {
        CheckSettlement::Stale {
            admission,
            outcome,
            check_result,
        } => {
            assert_eq!(admission.target, original);
            assert_eq!(outcome.exit_code, Some(1));
            assert!(!outcome.passed);
            assert!(outcome.verified);
            assert_eq!(check_result.value.id, check_result_id);
            assert_eq!(check_result.value.target, original);
            assert!(matches!(
                check_result.value.settlement,
                Some(CheckResultSettlement::Stale { .. })
            ));
        }
        CheckSettlement::Applied { .. } => panic!("a superseded step must not collect the result"),
    }
    assert_eq!(settled.freshness(), Some(&SourceFreshness::Unchanged));
    assert!(
        engines
            .run_steps()
            .unwrap()
            .iter()
            .all(|step| step.checks.is_empty()),
        "nothing lands on the new instance"
    );
}

#[test]
fn a_timeout_and_a_cancel_are_recorded_as_what_they_were() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (engines, root) = host(&tmp);
    goal(&engines, "verify");
    for execution in [CheckExecution::TimedOut, CheckExecution::Cancelled] {
        let admitted = AdmittedCheck::admit(&engines, &root, None, "test", "true".into(), true)
            .expect("admitted");
        let settled = admitted
            .settle(&engines, execution.clone(), String::new())
            .expect("settled");
        let check = settled.check().expect("recorded");
        assert!(!check.passed && check.verified);
        assert_eq!(check.exit_code, None);
        assert_eq!(settled.check_result().execution(), Some(&execution));
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
    goal(&engines, "verify");
    std::fs::remove_dir_all(&absent).expect("root disappears after host admission");
    let refused = AdmittedCheck::admit(&engines, &absent, None, "test", "true".into(), true)
        .expect_err("missing root");
    assert!(
        matches!(&refused, CheckEvidenceError::Source(reason) if reason.to_string().contains("does not exist")),
        "{refused}"
    );
    assert!(
        engines
            .run_steps()
            .unwrap()
            .iter()
            .all(|step| step.checks.is_empty()),
        "nothing was recorded"
    );
}

use std::collections::BTreeMap;

use magistr_engine::native::CompletionRefusal;

/// A check configuration whose standard checks and every named custom check
/// run `true`.
fn checks_json(custom: &[&str]) -> String {
    serde_json::json!({
        "project": { "name": "p", "description": "", "type": "cli" },
        "stack": {
            "language": "rust", "framework": "none", "package_manager": "cargo",
            "test_framework": "cargo-test", "linter": "clippy", "formatter": "rustfmt"
        },
        "checks": { "check": "true", "test": "true", "lint": "true" },
        "format": { "extensions": ["rs"], "command": "true" },
        "testing": { "companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside" },
        "reference_repos": [],
        "state_prefix": "p",
        "custom_checks": custom
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
    std::fs::write(root.join(".magistr/project.json"), checks_json(&[])).expect("checks");
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

fn passing_check(engines: &NativeEngines, root: &Path, name: &str) -> SettledCheck {
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
        .map(|check| (check.check.clone(), check.reason.clone()))
        .collect()
}

#[test]
fn an_external_policy_or_the_command_scope_edited_around_a_check_is_evidence_drift() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (engines, root, policy) = governed_host(&tmp);
    goal(&engines, "verify");
    for name in ["check", "test", "lint"] {
        let settled = passing_check(&engines, &root, name);
        assert_eq!(settled.freshness(), Some(&SourceFreshness::Unchanged));
    }
    let evidence = engines
        .run_verify_completion("task:test", Some("verify"))
        .expect("every required check has current evidence");
    assert!(evidence.is_complete());
    assert!(!evidence.config_digest.is_empty());

    // The organization policy changes after the checks ran; no source byte
    // moves. The evidence no longer applies, and says why.
    std::fs::write(&policy, "[security]\nredaction=false\n").expect("policy edit");
    let refusal = engines
        .run_verify_completion("task:test", Some("verify"))
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
            .contains("the applicable external configuration changed since the check ran")),
        "{refusal}"
    );

    // A policy edited while a check runs is named at settlement.
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
        .run_verify_completion("task:test", Some("verify"))
        .expect_err("a reconfiguration during the run is not evidence");
    assert!(
        outstanding(&refusal)["check"]
            .contains("configuration changed while the check ran: org-policy"),
        "{refusal}"
    );

    // Re-run under the current policy: the latest attempts govern.
    for name in ["check", "test", "lint"] {
        passing_check(&engines, &root, name);
    }
    assert!(
        engines
            .run_verify_completion("task:test", Some("verify"))
            .is_ok()
    );

    // The command scope widens after the checks: the check configuration is a
    // source input, so the old check_results read as a source change, and the
    // new contract has no check_result at all.
    std::fs::write(
        root.join(".magistr/project.json"),
        checks_json(&["structure"]),
    )
    .expect("scope edit");
    let refusal = engines
        .run_verify_completion("task:test", Some("verify"))
        .expect_err("a widened command scope invalidates the evidence");
    let reasons = outstanding(&refusal);
    assert!(
        reasons["structure"].contains("no check_result"),
        "{refusal}"
    );
    assert!(
        reasons["check"].contains("the source changed since the check ran"),
        "{refusal}"
    );
    for name in ["check", "test", "lint", "structure"] {
        passing_check(&engines, &root, name);
    }
    assert!(
        engines
            .run_verify_completion("task:test", Some("verify"))
            .is_ok()
    );
}

#[tokio::test]
async fn real_host_reuses_narrowed_inputs_and_promotes_evidence_for_completion() {
    let tmp = tempfile::tempdir().unwrap();
    let (engines, root) = host(&tmp);
    std::fs::write(root.join("Cargo.toml"),
        "[package]\nname = \"evidence-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[[bin]]\nname = \"fixture\"\npath = \"main.rs\"\n").unwrap();
    std::fs::write(root.join("main.rs"), "fn main() {}\n").unwrap();
    let command = "cargo fmt --all -- --check";
    let config = serde_json::json!({
        "project":{"name":"test","description":"","type":"cli"},
        "stack":{"language":"rust","framework":"none","package_manager":"cargo","test_framework":"cargo-test","linter":"clippy","formatter":"rustfmt"},
        "checks":{"check":command,"test":"cargo test","lint":"cargo clippy"},
        "format":{"extensions":["rs"],"command":"cargo fmt"},
        "testing":{"companion_pattern":"{name}.tests.{ext}","test_dir":"alongside"},
        "reference_repos":[],"state_prefix":"test"
    });
    std::fs::write(root.join(".magistr/project.json"), config.to_string()).unwrap();
    engines
        .run_open_with_verification(
            "Reuse current evidence".into(),
            vec![],
            vec![],
            "task:test".into(),
            magistr_engine::native::VerificationMode::Focused,
        )
        .unwrap();
    engines
        .add_step("verify".into(), "Verify".into(), StepType::Test, None)
        .unwrap();
    engines.start_step("verify").unwrap();
    let first =
        AdmittedCheck::admit(&engines, &root, None, "check", command.into(), false).unwrap();
    let run = crate::utils::check::run_check(
        "check",
        command,
        false,
        &root,
        30,
        &tokio_util::sync::CancellationToken::new(),
        |_| {},
    )
    .await;
    assert!(run.execution().passed(), "{}", run.details());
    let original = first
        .settle(&engines, run.execution(), run.details())
        .unwrap();
    std::fs::write(root.join("notes.txt"), "changed unrelated metadata").unwrap();
    let second = AdmittedCheck::admit_with_options(
        &engines,
        &root,
        None,
        "check",
        command.into(),
        true,
        magistr_engine::native::CheckOptions {
            inputs: vec!["Cargo.toml".into(), "main.rs".into()],
            reuse_only: true,
            ..Default::default()
        },
    )
    .unwrap();
    let reused = second
        .reuse(&engines)
        .unwrap()
        .expect("the runner must not be called twice");
    assert!(
        matches!(reused.check_result().execution(), Some(CheckExecution::Reused { check_result_id })
        if check_result_id == &original.check_result().id)
    );
    assert!(
        engines
            .run_verify_completion("task:test", Some("verify"))
            .unwrap()
            .is_complete()
    );

    std::fs::write(root.join("main.rs"), "fn main() { panic!() }\n").unwrap();
    let changed = AdmittedCheck::admit_with_options(
        &engines,
        &root,
        None,
        "check",
        command.into(),
        true,
        magistr_engine::native::CheckOptions {
            inputs: vec!["Cargo.toml".into(), "main.rs".into()],
            reuse_only: true,
            ..Default::default()
        },
    )
    .unwrap();
    let refused = changed
        .reuse(&engines)
        .unwrap()
        .expect("settled non-execution");
    assert!(matches!(
        refused.check_result().execution(),
        Some(CheckExecution::ReuseUnavailable { .. })
    ));
    assert!(!refused.check_result().verified());
    assert!(!refused.check().unwrap().verified);
    let report = root.join("target/nextest/default/junit.xml");
    std::fs::create_dir_all(report.parent().unwrap()).unwrap();
    std::fs::write(
        &report,
        "<testsuite tests=\"1\" failures=\"0\"><testcase name=\"stale\"/></testsuite>",
    )
    .unwrap();
    let absent = AdmittedCheck::admit_with_options(
        &engines,
        &root,
        None,
        "test",
        "cargo nextest run".into(),
        true,
        magistr_engine::native::CheckOptions {
            inputs: vec!["Cargo.toml".into(), "main.rs".into()],
            reuse_only: true,
            ..Default::default()
        },
    )
    .unwrap()
    .reuse(&engines)
    .unwrap()
    .expect("non-execution is settled");
    let check = absent.check().unwrap();
    assert!(!check.verified && !check.passed);
    assert!(
        check.report.is_none() && check.results.is_none(),
        "an old report cannot claim tests for a command that never ran"
    );
    assert!(
        engines
            .run_verify_completion("task:test", Some("verify"))
            .is_err()
    );
}
