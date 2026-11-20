//! Durable check check_results and evidence-governed completion.
//!
//! A fake evidence port stands in for the host's source capability: it
//! answers a revision the test controls and the check contracts the test
//! declares, so every rule of completion can be driven without a tree.

use std::sync::{Arc, Mutex};

use magistr_workflow::run::application::{
    CheckContract, CheckExecution, CheckOutcome, CheckRequest, CheckSettlement, CompletionRefusal,
    EvidencePort, RunUseCases, SourceFreshness, SourceObservation, SourceWitness,
};
use magistr_workflow::run::domain::{check::CheckType, goal::VerificationMode, step::StepType};
use magistr_workflow::run::engine::RunEngine;
use magistr_workflow::run::persistence::{Persistence, PersistenceConfig};

/// What one observation of the fake world saw: the source bytes, the
/// external configuration, the selection policy and the owner generation,
/// each its own identity as the real capability keeps them.
#[derive(Clone, PartialEq, Eq)]
struct Seen {
    revision: String,
    config: String,
    execution: String,
    policy: String,
    generation: Option<u64>,
    /// `false` mimics a check_result recorded before digests were bound.
    bound: bool,
}

/// The world as the fake port sees it: one mutable source revision, one
/// external configuration, one selection policy, one owner generation and
/// the checks the project requires.
struct World {
    seen: Mutex<Seen>,
    checks: Mutex<Vec<CheckContract>>,
    refuse: Mutex<Option<String>>,
}

impl Default for World {
    fn default() -> Self {
        Self {
            seen: Mutex::new(Seen {
                revision: String::new(),
                config: "c1".into(),
                execution: "toolchain-env-1".into(),
                policy: "p".into(),
                generation: Some(3),
                bound: true,
            }),
            checks: Mutex::new(Vec::new()),
            refuse: Mutex::new(None),
        }
    }
}

impl World {
    fn set_revision(&self, revision: &str) {
        self.seen.lock().unwrap().revision = revision.to_owned();
    }
    fn set_config(&self, config: &str) {
        self.seen.lock().unwrap().config = config.to_owned();
    }
    fn set_policy(&self, policy: &str) {
        self.seen.lock().unwrap().policy = policy.to_owned();
    }
    fn set_generation(&self, generation: u64) {
        self.seen.lock().unwrap().generation = Some(generation);
    }
    fn set_bound(&self, bound: bool) {
        self.seen.lock().unwrap().bound = bound;
    }
    fn now(&self) -> Seen {
        self.seen.lock().unwrap().clone()
    }
    fn require(&self, name: &str, command: &str) {
        self.checks.lock().unwrap().push(CheckContract {
            name: name.into(),
            command: command.into(),
            check_type: CheckType::Test,
        });
    }
}

struct Witness {
    world: Arc<World>,
    seen: Seen,
}

impl SourceWitness for Witness {
    fn observation(&self) -> SourceObservation {
        // The workspace's revision folds the owner generation in, exactly
        // as the real snapshot's does; the digests do not.
        SourceObservation {
            inputs: Vec::new(),
            execution_digest: if self.seen.bound {
                self.seen.execution.clone()
            } else {
                String::new()
            },
            workspace_root: "/w".into(),
            workspace_generation: self.seen.generation,
            scheme: "fake/1".into(),
            policy_digest: self.seen.policy.clone(),
            revision: format!("{}@{:?}", self.seen.revision, self.seen.generation),
            source_digest: if self.seen.bound {
                self.seen.revision.clone()
            } else {
                String::new()
            },
            config_digest: if self.seen.bound {
                self.seen.config.clone()
            } else {
                String::new()
            },
            consistency: "optimistic".into(),
            captured_at_ms: 0,
        }
    }
    fn validate(&self) -> Result<SourceFreshness, String> {
        let now = self.world.now();
        Ok(if now.revision != self.seen.revision {
            SourceFreshness::Changed {
                paths: vec!["src/lib.rs".into()],
            }
        } else if now.config != self.seen.config || now.execution != self.seen.execution {
            SourceFreshness::Reconfigured {
                layers: vec!["org-policy".into()],
            }
        } else {
            SourceFreshness::Unchanged
        })
    }
}

impl EvidencePort for World {
    fn observe(&self) -> Result<Box<dyn SourceWitness>, String> {
        if let Some(reason) = self.refuse.lock().unwrap().clone() {
            return Err(reason);
        }
        let seen = self.now();
        Ok(Box::new(Witness {
            world: Arc::new(World {
                seen: Mutex::new(seen.clone()),
                checks: Mutex::new(Vec::new()),
                refuse: Mutex::new(None),
            }),
            seen,
        }))
    }
    fn required_checks(&self) -> Result<Vec<CheckContract>, String> {
        Ok(self.checks.lock().unwrap().clone())
    }
}

/// A port whose witnesses see the live world (so a change during the run
/// is reported at settlement).
struct LivePort(Arc<World>);

impl EvidencePort for LivePort {
    fn can_reuse(&self, request: &CheckRequest) -> bool {
        request.command.is_some()
    }
    fn observe(&self) -> Result<Box<dyn SourceWitness>, String> {
        if let Some(reason) = self.0.refuse.lock().unwrap().clone() {
            return Err(reason);
        }
        Ok(Box::new(Witness {
            world: Arc::clone(&self.0),
            seen: self.0.now(),
        }))
    }
    fn required_checks(&self) -> Result<Vec<CheckContract>, String> {
        self.0.required_checks()
    }
}

fn stored(root: &std::path::Path, world: &Arc<World>) -> RunUseCases {
    let persistence = Persistence::new(&PersistenceConfig {
        enabled: true,
        data_dir: root.into(),
    });
    RunUseCases::new(Arc::new(Mutex::new(
        RunEngine::new("project".into()).with_persistence(persistence),
    )))
    .with_evidence(Arc::new(LivePort(Arc::clone(world))))
}

fn planned(cases: &RunUseCases) {
    cases
        .set_goal_with_verification(
            "Current run".into(),
            vec!["it works".into()],
            vec![],
            "task:c1".into(),
            VerificationMode::Full,
        )
        .unwrap();
    cases
        .add_step("verify".into(), "Verify".into(), StepType::Test, None, None)
        .unwrap();
    cases.start_step("verify").unwrap();
}

fn planned_focused(cases: &RunUseCases) {
    cases
        .set_goal_with_verification(
            "Docs-only run".into(),
            vec!["the docs are accurate".into()],
            vec![],
            "task:c1".into(),
            VerificationMode::Focused,
        )
        .unwrap();
    cases
        .add_step(
            "verify".into(),
            "Verify docs".into(),
            StepType::Docs,
            None,
            None,
        )
        .unwrap();
    cases.start_step("verify").unwrap();
}

fn request(check: &str, command: &str) -> CheckRequest {
    CheckRequest {
        options: Default::default(),
        step_id: None,
        check_type: CheckType::Test,
        name: check.into(),
        passed: None,
        details: String::new(),
        required: true,
        command: Some(command.into()),
        report: None,
    }
}

fn outcome(execution: CheckExecution, command: &str) -> CheckOutcome {
    CheckOutcome {
        passed: execution.passed(),
        details: String::new(),
        verified: execution.verified(),
        command: Some(command.into()),
        exit_code: execution.exit_code(),
        execution,
        report: None,
        results: None,
    }
}

fn run(
    cases: &RunUseCases,
    check: &str,
    command: &str,
    execution: CheckExecution,
) -> CheckSettlement {
    let admission = cases.admit_check(request(check, command)).unwrap().value;
    cases
        .settle_check(admission, outcome(execution, command))
        .unwrap()
}

fn world() -> Arc<World> {
    let world = Arc::new(World::default());
    world.set_revision("r1");
    world.require("test", "cargo test");
    world
}

#[test]
fn focused_goal_accepts_recorded_acceptance_checks_without_project_checks() {
    let temp = tempfile::tempdir().unwrap();
    let world = world();
    let cases = stored(temp.path(), &world);
    planned_focused(&cases);

    run(
        &cases,
        "docs",
        "docs check",
        CheckExecution::Exited { code: 0 },
    );

    let evidence = cases.verify_completion("task:c1", Some("verify")).unwrap();
    assert!(evidence.is_complete());
    assert_eq!(
        evidence
            .accepted
            .iter()
            .map(|check_result| check_result.check.as_str())
            .collect::<Vec<_>>(),
        vec!["docs"]
    );
}

#[test]
fn admission_publishes_a_durable_check_result_and_settlement_completes_it() {
    let temp = tempfile::tempdir().unwrap();
    let world = world();
    let cases = stored(temp.path(), &world);
    planned(&cases);
    let admission = cases
        .admit_check(request("test", "cargo test"))
        .unwrap()
        .value;
    let ledger = cases.engine().lock().unwrap().check_results().clone();
    let admitted = ledger.get(&admission.check_result_id).unwrap().unwrap();
    assert_eq!(admitted.attempt, 1);
    assert_eq!(admitted.target, admission.target);
    let source = admitted.source.as_ref().unwrap();
    assert_eq!(source.source_digest, "r1");
    assert_eq!(source.config_digest, "c1");
    assert_eq!(source.revision, "r1@Some(3)");
    assert_eq!(admitted.acceptance_revision, admission.acceptance_revision);
    assert!(admitted.execution().is_none() && admitted.settlement.is_none());
    assert!(
        ledger
            .directory()
            .unwrap()
            .join(format!("{}.json", admission.check_result_id))
            .is_file()
    );

    let settled = cases
        .settle_check(
            admission,
            outcome(CheckExecution::Exited { code: 0 }, "cargo test"),
        )
        .unwrap();
    let CheckSettlement::Applied {
        check,
        check_result,
    } = settled
    else {
        panic!("current instance applies");
    };
    assert!(check.value.verified && check.value.passed);
    assert!(
        check_result.value.is_applied()
            && check_result.value.passed()
            && check_result.value.verified()
    );
    assert_eq!(
        check_result.value.freshness,
        Some(SourceFreshness::Unchanged)
    );
    assert!(check_result.value.settled_at.is_some() && check_result.value.duration_ms.is_some());
    assert_eq!(
        ledger.get(&check_result.value.id).unwrap().unwrap(),
        check_result.value
    );
    // A fresh engine over the same store reads the same check_result.
    let again = stored(temp.path(), &world);
    let run = again.admit_step(None).unwrap().run_id;
    assert_eq!(
        again
            .engine()
            .lock()
            .unwrap()
            .check_results()
            .for_run(&run)
            .unwrap(),
        vec![check_result.value]
    );
}

#[test]
fn attempts_count_per_instance_and_check_and_a_change_during_the_run_is_recorded() {
    let temp = tempfile::tempdir().unwrap();
    let world = world();
    let cases = stored(temp.path(), &world);
    planned(&cases);
    run(
        &cases,
        "test",
        "cargo test",
        CheckExecution::Exited { code: 1 },
    );
    let admission = cases
        .admit_check(request("test", "cargo test"))
        .unwrap()
        .value;
    world.set_revision("r2");
    let settled = cases
        .settle_check(
            admission,
            outcome(CheckExecution::Exited { code: 0 }, "cargo test"),
        )
        .unwrap();
    let check_result = settled.check_result();
    assert_eq!(check_result.value.attempt, 2);
    assert_eq!(
        check_result.value.freshness,
        Some(SourceFreshness::Changed {
            paths: vec!["src/lib.rs".into()]
        })
    );
    let lint = run(
        &cases,
        "lint",
        "cargo clippy",
        CheckExecution::Exited { code: 0 },
    );
    assert_eq!(
        lint.check_result().value.attempt,
        1,
        "another check counts on its own"
    );
}

#[test]
fn every_execution_outcome_is_recorded_and_none_but_exit_zero_certifies() {
    let temp = tempfile::tempdir().unwrap();
    let world = world();
    let cases = stored(temp.path(), &world);
    planned(&cases);
    for (execution, fragment) in [
        (CheckExecution::TimedOut, "timed out"),
        (CheckExecution::Cancelled, "cancelled"),
        (
            CheckExecution::LaunchFailed {
                reason: "no such file".into(),
            },
            "launch failed",
        ),
        (CheckExecution::Exited { code: 101 }, "exit 101"),
    ] {
        let settled = run(&cases, "test", "cargo test", execution.clone());
        assert_eq!(settled.check_result().value.execution(), Some(&execution));
        let refusal = cases.verify_completion("task:c1", None).unwrap_err();
        let CompletionRefusal::Outstanding(evidence) = &refusal else {
            panic!("{refusal}");
        };
        assert_eq!(evidence.outstanding.len(), 1);
        assert!(
            evidence.outstanding[0].reason.contains(fragment),
            "{}",
            evidence.outstanding[0].reason
        );
    }
}

#[test]
fn completion_accepts_current_evidence_and_separates_recovered_from_outstanding() {
    let temp = tempfile::tempdir().unwrap();
    let world = world();
    world.require("lint", "cargo clippy");
    let cases = stored(temp.path(), &world);
    planned(&cases);
    run(
        &cases,
        "test",
        "cargo test",
        CheckExecution::Exited { code: 1 },
    );
    run(
        &cases,
        "test",
        "cargo test",
        CheckExecution::Exited { code: 0 },
    );
    let refusal = cases.verify_completion("task:c1", None).unwrap_err();
    let CompletionRefusal::Outstanding(evidence) = &refusal else {
        panic!("{refusal}");
    };
    assert_eq!(evidence.outstanding[0].check, "lint");
    assert!(evidence.outstanding[0].reason.contains("no check_result"));
    assert_eq!(evidence.recovered.len(), 1);
    assert_eq!(evidence.recovered[0].outcome, "exit 1");
    assert!(
        refusal
            .to_string()
            .contains("lint: required check has no check_result")
    );

    run(
        &cases,
        "lint",
        "cargo clippy",
        CheckExecution::Exited { code: 0 },
    );
    let evidence = cases.verify_completion("task:c1", Some("verify")).unwrap();
    assert!(evidence.is_complete());
    assert_eq!(evidence.step_id, "verify");
    assert_eq!(evidence.source_revision, "r1@Some(3)");
    assert_eq!(evidence.config_digest, "c1");
    assert_eq!(
        evidence
            .accepted
            .iter()
            .map(|a| (a.check.as_str(), a.attempt))
            .collect::<Vec<_>>(),
        vec![("test", 2), ("lint", 1)]
    );
    assert_eq!(evidence.recovered.len(), 1);

    // The finalize report tells the same story.
    let report = cases.finalize(vec![], None).unwrap().value;
    assert_eq!(report.recovered_failures, vec!["test (step: verify)"]);
    assert!(report.outstanding_failures.is_empty());
    assert!(
        !report
            .warnings
            .iter()
            .any(|w| w.starts_with("required checks failed:"))
    );
}

#[test]
fn completion_refuses_evidence_taken_under_another_source_or_acceptance_or_command() {
    let temp = tempfile::tempdir().unwrap();
    let world = world();
    let cases = stored(temp.path(), &world);
    planned(&cases);
    run(
        &cases,
        "test",
        "cargo test",
        CheckExecution::Exited { code: 0 },
    );
    assert!(cases.verify_completion("task:c1", None).is_ok());

    // The source moved on after the check ran.
    world.set_revision("r2");
    let refusal = cases.verify_completion("task:c1", None).unwrap_err();
    assert!(
        refusal
            .to_string()
            .contains("source changed since the check ran"),
        "{refusal}"
    );
    world.set_revision("r1");
    assert!(cases.verify_completion("task:c1", None).is_ok());

    // The acceptance was edited mid-flight (same run).
    cases
        .set_goal_with_verification(
            "Current run".into(),
            vec!["it works".into(), "and it is fast".into()],
            vec![],
            "task:c1".into(),
            VerificationMode::Full,
        )
        .unwrap();
    let refusal = cases.verify_completion("task:c1", None).unwrap_err();
    assert!(
        refusal.to_string().contains("acceptance changed"),
        "{refusal}"
    );
    run(
        &cases,
        "test",
        "cargo test",
        CheckExecution::Exited { code: 0 },
    );
    assert!(cases.verify_completion("task:c1", None).is_ok());

    // The configured command changed.
    world.checks.lock().unwrap()[0].command = "cargo nextest run".into();
    let refusal = cases.verify_completion("task:c1", None).unwrap_err();
    assert!(
        refusal.to_string().contains("not the configured"),
        "{refusal}"
    );

    // A foreign scope and a step outside the run are refused by name.
    assert!(matches!(
        cases.verify_completion("task:other", None).unwrap_err(),
        CompletionRefusal::ForeignScope { .. }
    ));
    assert!(matches!(
        cases
            .verify_completion("task:c1", Some("ghost"))
            .unwrap_err(),
        CompletionRefusal::StepNotCurrent { .. }
    ));
}

#[test]
fn a_self_reported_pass_never_certifies_a_configured_check() {
    let temp = tempfile::tempdir().unwrap();
    let world = world();
    let cases = stored(temp.path(), &world);
    planned(&cases);
    let mut claim = request("test", "cargo test");
    claim.command = None;
    claim.passed = Some(true);
    let admission = cases.admit_check(claim.clone()).unwrap().value;
    let execution = RunUseCases::self_reported(&claim);
    let mut reported = outcome(execution, "cargo test");
    reported.command = None;
    cases.settle_check(admission, reported).unwrap();
    let refusal = cases.verify_completion("task:c1", None).unwrap_err();
    assert!(refusal.to_string().contains("self-reported"), "{refusal}");
}

#[test]
fn late_settlement_after_rollover_stays_under_its_check_result_and_a_reused_name_starts_over() {
    let temp = tempfile::tempdir().unwrap();
    let world = world();
    let cases = stored(temp.path(), &world);
    planned(&cases);
    let admission = cases
        .admit_check(request("test", "cargo test"))
        .unwrap()
        .value;
    let first_run = admission.target.run_id.clone();
    cases.complete_step("verify", vec![]).unwrap();
    cases.finalize(vec![], None).unwrap();
    planned(&cases);
    let settled = cases
        .settle_check(
            admission.clone(),
            outcome(CheckExecution::Exited { code: 0 }, "cargo test"),
        )
        .unwrap();
    let CheckSettlement::Stale { check_result, .. } = settled else {
        panic!("a superseded run must not collect the result");
    };
    assert_eq!(check_result.value.id, admission.check_result_id);
    assert_eq!(check_result.value.target.run_id, first_run);
    assert!(check_result.value.passed() && !check_result.value.is_applied());
    let ledger = cases.engine().lock().unwrap().check_results().clone();
    assert_eq!(
        ledger.for_run(&first_run).unwrap(),
        vec![check_result.value]
    );

    // The new run's `verify` has no evidence of its own.
    let new_run = cases.admit_step(None).unwrap().run_id;
    assert!(ledger.for_run(&new_run).unwrap().is_empty());
    assert!(matches!(
        cases.verify_completion("task:c1", None).unwrap_err(),
        CompletionRefusal::NoCheckResults { .. }
    ));
    let fresh = run(
        &cases,
        "test",
        "cargo test",
        CheckExecution::Exited { code: 0 },
    );
    assert_eq!(fresh.check_result().value.attempt, 1);
    assert!(cases.verify_completion("task:c1", None).is_ok());
}

#[test]
fn without_an_evidence_capability_admission_still_works_and_completion_refuses() {
    let temp = tempfile::tempdir().unwrap();
    let persistence = Persistence::new(&PersistenceConfig {
        enabled: true,
        data_dir: temp.path().into(),
    });
    let cases = RunUseCases::new(Arc::new(Mutex::new(
        RunEngine::new("project".into()).with_persistence(persistence),
    )));
    planned(&cases);
    let settled = run(
        &cases,
        "test",
        "cargo test",
        CheckExecution::Exited { code: 0 },
    );
    assert!(settled.check_result().value.source.is_none());
    assert!(settled.check_result().value.is_applied());
    assert!(matches!(
        cases.verify_completion("task:c1", None).unwrap_err(),
        CompletionRefusal::NoEvidenceCapability
    ));
}

#[test]
fn a_refused_observation_refuses_admission_before_anything_runs() {
    let temp = tempfile::tempdir().unwrap();
    let world = world();
    let cases = stored(temp.path(), &world);
    planned(&cases);
    *world.refuse.lock().unwrap() = Some("selection past the ceiling".into());
    let error = cases
        .admit_check(request("test", "cargo test"))
        .unwrap_err();
    assert_eq!(error.kind(), "source_unavailable");
    assert_eq!(error.effect(), "none");
    let ledger = cases.engine().lock().unwrap().check_results().clone();
    let run_id = cases.admit_step(None).unwrap().run_id;
    assert!(ledger.for_run(&run_id).unwrap().is_empty());
}

#[test]
fn explicit_compaction_preserves_completion_and_late_results_across_rollover() {
    let temp = tempfile::tempdir().unwrap();
    let world = world();
    let old = stored(temp.path(), &world);
    planned(&old);
    let finished = run(
        &old,
        "test",
        "cargo test",
        CheckExecution::Exited { code: 0 },
    );
    let pending = old
        .admit_check(request("test", "cargo test"))
        .unwrap()
        .value;
    let original = old
        .engine()
        .lock()
        .unwrap()
        .check_results()
        .get(&pending.check_result_id)
        .unwrap()
        .unwrap();
    let newer = stored(temp.path(), &world);
    newer.reset().unwrap();
    planned(&newer);
    run(
        &newer,
        "test",
        "cargo test",
        CheckExecution::Exited { code: 1 },
    );
    run(
        &newer,
        "test",
        "cargo test",
        CheckExecution::Exited { code: 0 },
    );
    let before = newer.verify_completion("task:c1", None).unwrap();
    // The stale owner must preserve the newer persisted run, not its old cache.
    let report = old.compact_check_results().unwrap();
    assert_eq!(
        (
            report.removed,
            report.retained_current,
            report.retained_unsettled
        ),
        (1, 2, 1)
    );
    let ledger = newer.engine().lock().unwrap().check_results().clone();
    assert!(
        ledger
            .get(&finished.check_result().value.id)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        ledger.get(&pending.check_result_id).unwrap(),
        Some(original.clone())
    );
    let late = old
        .settle_check(
            pending,
            outcome(CheckExecution::Exited { code: 0 }, "cargo test"),
        )
        .unwrap();
    assert!(matches!(late, CheckSettlement::Stale { .. }));
    assert_eq!(late.check_result().value.attempt, original.attempt);
    assert_eq!(late.check_result().value.target, original.target);
    assert_eq!(
        serde_json::to_value(newer.verify_completion("task:c1", None).unwrap()).unwrap(),
        serde_json::to_value(before).unwrap()
    );
    assert_eq!(newer.compact_check_results().unwrap().removed, 1);
}

#[test]
fn completion_never_reads_poisoned_check_results_from_an_older_run() {
    let temp = tempfile::tempdir().unwrap();
    let world = world();
    let cases = stored(temp.path(), &world);
    planned(&cases);
    let old = run(
        &cases,
        "test",
        "cargo test",
        CheckExecution::Exited { code: 0 },
    );
    let directory = cases
        .engine()
        .lock()
        .unwrap()
        .check_results()
        .directory()
        .unwrap()
        .to_owned();
    cases.reset().unwrap();
    planned(&cases);
    run(
        &cases,
        "test",
        "cargo test",
        CheckExecution::Exited { code: 0 },
    );
    std::fs::write(
        directory.join(format!("{}.json", old.check_result().value.id)),
        b"damaged old check_result",
    )
    .unwrap();
    let reopened = stored(temp.path(), &world);
    assert!(reopened.verify_completion("task:c1", None).is_ok());
}

#[tokio::test]
async fn compaction_is_explicit_through_the_mcp_surface() {
    use magistr_workflow::run::mcp::{RunService, handlers::NoArgs};
    use rmcp::handler::server::wrapper::Parameters;
    let temp = tempfile::tempdir().unwrap();
    let world = world();
    let cases = stored(temp.path(), &world);
    let service = RunService::from_use_cases(cases.clone());
    let refused = service
        .run_compact_checks(Parameters(NoArgs {}))
        .await
        .unwrap();
    assert_eq!(refused.is_error, Some(true));
    planned(&cases);
    let old = run(
        &cases,
        "test",
        "cargo test",
        CheckExecution::Exited { code: 0 },
    );
    cases.reset().unwrap();
    planned(&cases);
    run(
        &cases,
        "test",
        "cargo test",
        CheckExecution::Exited { code: 0 },
    );
    assert!(
        cases
            .engine()
            .lock()
            .unwrap()
            .check_results()
            .get(&old.check_result().value.id)
            .unwrap()
            .is_some()
    );
    let result = service
        .run_compact_checks(Parameters(NoArgs {}))
        .await
        .unwrap();
    assert_ne!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content.unwrap(),
        serde_json::json!({
            "removed": 1, "retained_current": 1, "retained_unsettled": 0, "retained_referenced": 0, "storage": "file"
        })
    );
}

#[test]
fn an_external_layer_edited_while_the_check_ran_is_named_and_the_latest_attempt_governs() {
    let temp = tempfile::tempdir().unwrap();
    let world = world();
    let cases = stored(temp.path(), &world);
    planned(&cases);
    let admission = cases
        .admit_check(request("test", "cargo test"))
        .unwrap()
        .value;
    // The organization policy moved while the command ran; not one source
    // byte did.
    world.set_config("c2");
    let settled = cases
        .settle_check(
            admission,
            outcome(CheckExecution::Exited { code: 0 }, "cargo test"),
        )
        .unwrap();
    assert_eq!(
        settled.check_result().value.freshness,
        Some(SourceFreshness::Reconfigured {
            layers: vec!["org-policy".into()]
        })
    );
    let refusal = cases.verify_completion("task:c1", None).unwrap_err();
    assert!(
        refusal
            .to_string()
            .contains("configuration changed while the check ran: org-policy"),
        "{refusal}"
    );

    // Re-run under the current configuration: the latest attempt decides.
    let again = run(
        &cases,
        "test",
        "cargo test",
        CheckExecution::Exited { code: 0 },
    );
    assert_eq!(again.check_result().value.attempt, 2);
    assert_eq!(
        again
            .check_result()
            .value
            .source
            .as_ref()
            .unwrap()
            .config_digest,
        "c2"
    );
    let evidence = cases.verify_completion("task:c1", None).unwrap();
    assert_eq!(evidence.config_digest, "c2");
    assert_eq!(evidence.accepted[0].attempt, 2);
}

#[test]
fn completion_names_the_identity_that_drifted_after_the_check_and_ignores_the_owner_generation() {
    let temp = tempfile::tempdir().unwrap();
    let world = world();
    let cases = stored(temp.path(), &world);
    planned(&cases);
    run(
        &cases,
        "test",
        "cargo test",
        CheckExecution::Exited { code: 0 },
    );
    assert!(cases.verify_completion("task:c1", None).is_ok());

    // An external configuration layer changed after the check, every
    // selected byte unchanged: the evidence no longer applies.
    world.set_config("c2");
    let refusal = cases.verify_completion("task:c1", None).unwrap_err();
    assert!(
        refusal
            .to_string()
            .contains("the applicable external configuration changed since the check ran"),
        "{refusal}"
    );
    world.set_config("c1");
    assert!(cases.verify_completion("task:c1", None).is_ok());

    // The tools or the environment they read changed: named as that, never
    // as a configuration layer.
    world.seen.lock().unwrap().execution = "toolchain-env-2".into();
    let refusal = cases.verify_completion("task:c1", None).unwrap_err();
    assert!(
        refusal.to_string().contains(
            "the tools the check uses, or the environment variables they read, changed since the check ran"
        ),
        "{refusal}"
    );
    world.seen.lock().unwrap().execution = "toolchain-env-1".into();
    assert!(cases.verify_completion("task:c1", None).is_ok());

    // The selection policy changed: named as such, never as a source change.
    world.set_policy("p2");
    let refusal = cases.verify_completion("task:c1", None).unwrap_err();
    assert!(
        refusal
            .to_string()
            .contains("the selection policy changed since the check ran"),
        "{refusal}"
    );
    world.set_policy("p");
    assert!(cases.verify_completion("task:c1", None).is_ok());

    // A new owner of the same bytes moved the workspace's revision and
    // nothing else: the evidence still applies.
    world.set_generation(4);
    let evidence = cases.verify_completion("task:c1", None).unwrap();
    assert_ne!(
        evidence.source_revision,
        evidence.accepted[0].source_revision.clone().unwrap(),
        "the workspace revision folds the generation in"
    );
    assert!(evidence.is_complete());

    // The source itself moved: named as a source change.
    world.set_revision("r2");
    let refusal = cases.verify_completion("task:c1", None).unwrap_err();
    assert!(
        refusal
            .to_string()
            .contains("the source changed since the check ran"),
        "{refusal}"
    );
}

#[test]
fn a_check_result_recorded_before_digests_were_bound_is_refused_by_name() {
    let temp = tempfile::tempdir().unwrap();
    let world = world();
    let cases = stored(temp.path(), &world);
    planned(&cases);
    world.set_bound(false);
    run(
        &cases,
        "test",
        "cargo test",
        CheckExecution::Exited { code: 0 },
    );
    world.set_bound(true);
    let refusal = cases.verify_completion("task:c1", None).unwrap_err();
    assert!(
        refusal
            .to_string()
            .contains("recorded before source evidence was bound"),
        "{refusal}"
    );
    run(
        &cases,
        "test",
        "cargo test",
        CheckExecution::Exited { code: 0 },
    );
    assert!(cases.verify_completion("task:c1", None).is_ok());
}

#[test]
fn a_settlement_without_the_witness_compares_digests_not_generations() {
    let temp = tempfile::tempdir().unwrap();
    let world = world();
    let cases = stored(temp.path(), &world);
    planned(&cases);

    // Admitted here, settled by another process (a fresh handle over the
    // same store holds no witness): only the observations are compared.
    let admission = cases
        .admit_check(request("test", "cargo test"))
        .unwrap()
        .value;
    world.set_generation(4);
    let elsewhere = stored(temp.path(), &world);
    let settled = elsewhere
        .settle_check(
            admission,
            outcome(CheckExecution::Exited { code: 0 }, "cargo test"),
        )
        .unwrap();
    assert_eq!(
        settled.check_result().value.freshness,
        Some(SourceFreshness::Unchanged)
    );

    let admission = cases
        .admit_check(request("test", "cargo test"))
        .unwrap()
        .value;
    world.set_config("c2");
    let settled = elsewhere
        .settle_check(
            admission,
            outcome(CheckExecution::Exited { code: 0 }, "cargo test"),
        )
        .unwrap();
    assert_eq!(
        settled.check_result().value.freshness,
        Some(SourceFreshness::Reconfigured { layers: vec![] })
    );

    let admission = cases
        .admit_check(request("test", "cargo test"))
        .unwrap()
        .value;
    world.set_revision("r2");
    let settled = elsewhere
        .settle_check(
            admission,
            outcome(CheckExecution::Exited { code: 0 }, "cargo test"),
        )
        .unwrap();
    assert_eq!(
        settled.check_result().value.freshness,
        Some(SourceFreshness::Diverged {
            before: "r1".into(),
            after: "r2".into()
        })
    );
}

#[test]
fn check_result_compaction_keeps_the_checks_of_saved_unfinished_plans() {
    let temp = tempfile::tempdir().unwrap();
    let world = world();
    let cases = stored(temp.path(), &world);
    planned(&cases);
    let verified = run(
        &cases,
        "test",
        "cargo test",
        CheckExecution::Exited { code: 0 },
    );
    let evidence = serde_json::to_value(cases.verify_completion("task:c1", None).unwrap()).unwrap();
    cases
        .set_goal("Other work".into(), vec![], vec![], "task:c2".into())
        .unwrap();
    let report = cases.compact_check_results().unwrap();
    assert_eq!(report.removed, 0);
    assert_eq!(report.retained_referenced, 1);
    let reopened = stored(temp.path(), &world);
    reopened
        .set_goal(
            "Current run".into(),
            vec!["it works".into()],
            vec![],
            "task:c1".into(),
        )
        .unwrap();
    assert_eq!(
        serde_json::to_value(reopened.verify_completion("task:c1", None).unwrap()).unwrap(),
        evidence
    );
    assert!(
        reopened
            .engine()
            .lock()
            .unwrap()
            .check_results()
            .get(&verified.check_result().value.id)
            .unwrap()
            .is_some()
    );
}

#[test]
fn verified_inputs_reuse_across_steps_and_reopened_runs_without_relabeling_execution() {
    let dir = tempfile::tempdir().unwrap();
    let world = world();
    let cases = stored(dir.path(), &world);
    planned(&cases);
    let CheckSettlement::Applied { check_result, .. } = run(
        &cases,
        "test",
        "cargo test",
        CheckExecution::Exited { code: 0 },
    ) else {
        panic!("applied")
    };
    let original_id = check_result.value.id;
    cases.complete_step("verify", vec![]).unwrap();
    drop(cases);

    let reopened = stored(dir.path(), &world);
    reopened
        .add_step("second".into(), "Second".into(), StepType::Test, None, None)
        .unwrap();
    reopened.start_step("second").unwrap();
    let admission = reopened
        .admit_check(request("test", "cargo test"))
        .unwrap()
        .value;
    let reused = reopened
        .reuse_check(&admission)
        .unwrap()
        .expect("matching inputs");
    assert_eq!(
        reused.execution,
        CheckExecution::Reused {
            check_result_id: original_id.clone()
        }
    );
    assert_eq!(reused.exit_code, None, "no new process exit is claimed");
    reopened.settle_check(admission, reused).unwrap();
    reopened
        .verify_completion("task:c1", Some("second"))
        .unwrap();
    reopened.complete_step("second", vec![]).unwrap();

    // The input index is not tied to a goal or its step instances.
    reopened.reset().unwrap();
    planned_focused(&reopened);
    let admission = reopened
        .admit_check(request("test", "cargo test"))
        .unwrap()
        .value;
    let reused = reopened
        .reuse_check(&admission)
        .unwrap()
        .expect("new goal, same verified inputs");
    assert_eq!(
        reused.execution,
        CheckExecution::Reused {
            check_result_id: original_id
        }
    );
    reopened.settle_check(admission, reused).unwrap();
    reopened.compact_check_results().unwrap();
    let next = reopened
        .admit_check(request("test", "cargo test"))
        .unwrap()
        .value;
    assert!(
        reopened.reuse_check(&next).unwrap().is_some(),
        "compaction retains the referenced execution"
    );
}

#[test]
fn reuse_refuses_changed_inputs_configuration_environment_and_newer_failure() {
    for change in [
        "source",
        "config",
        "policy",
        "environment",
        "command",
        "forced",
        "failure",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let world = world();
        let cases = stored(dir.path(), &world);
        planned(&cases);
        run(
            &cases,
            "test",
            "cargo test",
            CheckExecution::Exited { code: 0 },
        );
        let mut check = request("test", "cargo test");
        match change {
            "source" => world.set_revision("r2"),
            "config" => world.set_config("c2"),
            "policy" => world.set_policy("p2"),
            "environment" => world.seen.lock().unwrap().execution = "toolchain-env-2".into(),
            "command" => check.command = Some("cargo test --features new".into()),
            "forced" => check.options.reuse = false,
            "failure" => {
                run(
                    &cases,
                    "test",
                    "cargo test",
                    CheckExecution::Exited { code: 7 },
                );
            }
            _ => unreachable!(),
        }
        let admission = cases.admit_check(check).unwrap().value;
        assert!(cases.reuse_check(&admission).unwrap().is_none(), "{change}");
    }
}

#[test]
fn a_reused_result_cannot_claim_an_unverified_or_foreign_check_result() {
    let dir = tempfile::tempdir().unwrap();
    let world = world();
    let cases = stored(dir.path(), &world);
    planned(&cases);
    let admission = cases
        .admit_check(request("test", "cargo test"))
        .unwrap()
        .value;
    let error = cases
        .settle_check(
            admission,
            outcome(
                CheckExecution::Reused {
                    check_result_id: "does-not-exist".into(),
                },
                "cargo test",
            ),
        )
        .unwrap_err();
    assert_eq!(error.kind(), "invalid_reuse");
}

#[test]
fn optional_evidence_remains_reusable_after_promotion_and_elapsed_time() {
    let dir = tempfile::tempdir().unwrap();
    let world = world();
    let cases = stored(dir.path(), &world);
    planned_focused(&cases);
    let mut check = request("test", "cargo test");
    check.required = false;
    let admission = cases.admit_check(check).unwrap().value;
    let original = cases
        .settle_check(
            admission,
            outcome(CheckExecution::Exited { code: 0 }, "cargo test"),
        )
        .unwrap();
    let id = original.check_result().value.id.clone();
    let path = cases
        .engine()
        .lock()
        .unwrap()
        .check_results()
        .directory()
        .unwrap()
        .join(format!("{id}.json"));
    let mut record: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    record["admitted_at"] =
        serde_json::json!((chrono::Utc::now() - chrono::Duration::days(2)).to_rfc3339());
    std::fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
    let admission = cases
        .admit_check(request("test", "cargo test"))
        .unwrap()
        .value;
    let reused = cases
        .reuse_check(&admission)
        .unwrap()
        .expect("same certified inputs");
    assert_eq!(
        reused.execution,
        CheckExecution::Reused {
            check_result_id: id
        }
    );
    cases.settle_check(admission, reused).unwrap();
    cases.verify_completion("task:c1", None).unwrap();
}

#[tokio::test]
async fn reuse_only_misses_settle_without_a_process_or_a_claimed_test_failure() {
    let dir = tempfile::tempdir().unwrap();
    let world = world();
    let cases = stored(dir.path(), &world);
    planned_focused(&cases);
    let mut check = request("test", "a-command-that-must-never-start");
    check.options.reuse_only = true;
    let admission = cases.admit_check(check).unwrap().value;
    let missing = cases
        .reuse_check(&admission)
        .unwrap()
        .expect("explicit no-run outcome");
    assert!(matches!(
        missing.execution,
        CheckExecution::ReuseUnavailable { .. }
    ));
    assert!(!missing.passed && !missing.verified && missing.exit_code.is_none());
    // A caller that bypasses lookup still cannot execute a reuse-only request.
    let direct = RunUseCases::execute_check(&admission).await;
    assert!(matches!(
        direct.execution,
        CheckExecution::ReuseUnavailable { .. }
    ));
    cases.settle_check(admission, missing).unwrap();
    assert!(cases.verify_completion("task:c1", None).is_err());
}
