//! Durable check receipts and evidence-governed completion.
//!
//! A fake evidence port stands in for the host's source capability: it
//! answers a revision the test controls and the gate contracts the test
//! declares, so every rule of completion can be driven without a tree.

use std::sync::{Arc, Mutex};

use magistr_workflow::ship::application::{
    CheckExecution, CheckOutcome, CheckRequest, CheckSettlement, CompletionRefusal, EvidencePort,
    GateContract, ShipUseCases, SourceFreshness, SourceObservation, SourceWitness,
};
use magistr_workflow::ship::domain::{check::CheckType, task::TaskType};
use magistr_workflow::ship::engine::ShipEngine;
use magistr_workflow::ship::persistence::{Persistence, PersistenceConfig};

/// The world as the fake port sees it: one mutable revision and the gates
/// the project requires.
#[derive(Default)]
struct World {
    revision: Mutex<String>,
    gates: Mutex<Vec<GateContract>>,
    refuse: Mutex<Option<String>>,
}

impl World {
    fn set_revision(&self, revision: &str) {
        *self.revision.lock().unwrap() = revision.to_owned();
    }
    fn require(&self, name: &str, command: &str) {
        self.gates.lock().unwrap().push(GateContract {
            name: name.into(),
            command: command.into(),
            check_type: CheckType::Test,
        });
    }
}

struct Witness {
    world: Arc<World>,
    seen: String,
}

impl SourceWitness for Witness {
    fn observation(&self) -> SourceObservation {
        SourceObservation {
            workspace_root: "/w".into(),
            workspace_generation: Some(3),
            scheme: "fake/1".into(),
            policy_digest: "p".into(),
            revision: self.seen.clone(),
            consistency: "optimistic".into(),
            captured_at_ms: 0,
        }
    }
    fn validate(&self) -> Result<SourceFreshness, String> {
        let now = self.world.revision.lock().unwrap().clone();
        Ok(if now == self.seen {
            SourceFreshness::Unchanged
        } else {
            SourceFreshness::Changed {
                paths: vec!["src/lib.rs".into()],
            }
        })
    }
}

impl EvidencePort for World {
    fn observe(&self) -> Result<Box<dyn SourceWitness>, String> {
        if let Some(reason) = self.refuse.lock().unwrap().clone() {
            return Err(reason);
        }
        Ok(Box::new(Witness {
            world: Arc::new(World {
                revision: Mutex::new(self.revision.lock().unwrap().clone()),
                gates: Mutex::new(Vec::new()),
                refuse: Mutex::new(None),
            }),
            seen: self.revision.lock().unwrap().clone(),
        }))
    }
    fn required_gates(&self) -> Result<Vec<GateContract>, String> {
        Ok(self.gates.lock().unwrap().clone())
    }
}

/// A port whose witnesses see the live world (so a change during the run
/// is reported at settlement).
struct LivePort(Arc<World>);

impl EvidencePort for LivePort {
    fn observe(&self) -> Result<Box<dyn SourceWitness>, String> {
        if let Some(reason) = self.0.refuse.lock().unwrap().clone() {
            return Err(reason);
        }
        Ok(Box::new(Witness {
            world: Arc::clone(&self.0),
            seen: self.0.revision.lock().unwrap().clone(),
        }))
    }
    fn required_gates(&self) -> Result<Vec<GateContract>, String> {
        self.0.required_gates()
    }
}

fn stored(root: &std::path::Path, world: &Arc<World>) -> ShipUseCases {
    let persistence = Persistence::new(&PersistenceConfig {
        enabled: true,
        data_dir: root.into(),
    });
    ShipUseCases::new(Arc::new(Mutex::new(
        ShipEngine::new("project".into()).with_persistence(persistence),
    )))
    .with_evidence(Arc::new(LivePort(Arc::clone(world))))
}

fn planned(cases: &ShipUseCases) {
    cases
        .set_objective(
            "Current run".into(),
            vec!["it works".into()],
            vec![],
            "chunk:c1".into(),
        )
        .unwrap();
    cases
        .add_task("verify".into(), "Verify".into(), TaskType::Test, None, None)
        .unwrap();
    cases.start_task("verify").unwrap();
}

fn request(gate: &str, command: &str) -> CheckRequest {
    CheckRequest {
        task_id: None,
        check_type: CheckType::Test,
        name: gate.into(),
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
    cases: &ShipUseCases,
    gate: &str,
    command: &str,
    execution: CheckExecution,
) -> CheckSettlement {
    let admission = cases.admit_check(request(gate, command)).unwrap();
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
fn admission_publishes_a_durable_receipt_and_settlement_completes_it() {
    let temp = tempfile::tempdir().unwrap();
    let world = world();
    let cases = stored(temp.path(), &world);
    planned(&cases);
    let admission = cases.admit_check(request("test", "cargo test")).unwrap();
    let ledger = cases.engine().lock().unwrap().receipts().clone();
    let admitted = ledger.get(&admission.receipt_id).unwrap().unwrap();
    assert_eq!(admitted.attempt, 1);
    assert_eq!(admitted.target, admission.target);
    assert_eq!(admitted.source.as_ref().unwrap().revision, "r1");
    assert_eq!(admitted.acceptance_revision, admission.acceptance_revision);
    assert!(admitted.execution.is_none() && admitted.settlement.is_none());
    assert!(
        ledger
            .directory()
            .unwrap()
            .join(format!("{}.json", admission.receipt_id))
            .is_file()
    );

    let settled = cases
        .settle_check(
            admission,
            outcome(CheckExecution::Exited { code: 0 }, "cargo test"),
        )
        .unwrap();
    let CheckSettlement::Applied { check, receipt } = settled else {
        panic!("current instance applies");
    };
    assert!(check.verified && check.passed);
    assert!(receipt.is_applied() && receipt.passed() && receipt.verified());
    assert_eq!(receipt.freshness, Some(SourceFreshness::Unchanged));
    assert!(receipt.settled_at.is_some() && receipt.duration_ms.is_some());
    assert_eq!(ledger.get(&receipt.id).unwrap().unwrap(), *receipt);
    // A fresh engine over the same store reads the same receipt.
    let again = stored(temp.path(), &world);
    let run = again.admit_task(None).unwrap().run_id;
    assert_eq!(
        again
            .engine()
            .lock()
            .unwrap()
            .receipts()
            .for_run(&run)
            .unwrap(),
        vec![*receipt]
    );
}

#[test]
fn attempts_count_per_instance_and_gate_and_a_change_during_the_run_is_recorded() {
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
    let admission = cases.admit_check(request("test", "cargo test")).unwrap();
    world.set_revision("r2");
    let settled = cases
        .settle_check(
            admission,
            outcome(CheckExecution::Exited { code: 0 }, "cargo test"),
        )
        .unwrap();
    let receipt = settled.receipt();
    assert_eq!(receipt.attempt, 2);
    assert_eq!(
        receipt.freshness,
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
    assert_eq!(lint.receipt().attempt, 1, "another gate counts on its own");
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
        assert_eq!(settled.receipt().execution, Some(execution));
        let refusal = cases.verify_completion("chunk:c1", None).unwrap_err();
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
    let refusal = cases.verify_completion("chunk:c1", None).unwrap_err();
    let CompletionRefusal::Outstanding(evidence) = &refusal else {
        panic!("{refusal}");
    };
    assert_eq!(evidence.outstanding[0].gate, "lint");
    assert!(evidence.outstanding[0].reason.contains("no receipt"));
    assert_eq!(evidence.recovered.len(), 1);
    assert_eq!(evidence.recovered[0].outcome, "exit 1");
    assert!(
        refusal
            .to_string()
            .contains("lint: required gate has no receipt")
    );

    run(
        &cases,
        "lint",
        "cargo clippy",
        CheckExecution::Exited { code: 0 },
    );
    let evidence = cases.verify_completion("chunk:c1", Some("verify")).unwrap();
    assert!(evidence.is_complete());
    assert_eq!(evidence.task_id, "verify");
    assert_eq!(evidence.source_revision, "r1");
    assert_eq!(
        evidence
            .accepted
            .iter()
            .map(|a| (a.gate.as_str(), a.attempt))
            .collect::<Vec<_>>(),
        vec![("test", 2), ("lint", 1)]
    );
    assert_eq!(evidence.recovered.len(), 1);

    // The finalize report tells the same story.
    let report = cases.finalize(vec![], None).unwrap();
    assert_eq!(report.recovered_failures, vec!["test (task: verify)"]);
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
    assert!(cases.verify_completion("chunk:c1", None).is_ok());

    // The source moved on after the gate ran.
    world.set_revision("r2");
    let refusal = cases.verify_completion("chunk:c1", None).unwrap_err();
    assert!(
        refusal
            .to_string()
            .contains("source changed since the gate ran"),
        "{refusal}"
    );
    world.set_revision("r1");
    assert!(cases.verify_completion("chunk:c1", None).is_ok());

    // The acceptance was edited mid-flight (same run).
    cases
        .set_objective(
            "Current run".into(),
            vec!["it works".into(), "and it is fast".into()],
            vec![],
            "chunk:c1".into(),
        )
        .unwrap();
    let refusal = cases.verify_completion("chunk:c1", None).unwrap_err();
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
    assert!(cases.verify_completion("chunk:c1", None).is_ok());

    // The configured command changed.
    world.gates.lock().unwrap()[0].command = "cargo nextest run".into();
    let refusal = cases.verify_completion("chunk:c1", None).unwrap_err();
    assert!(
        refusal.to_string().contains("not the configured"),
        "{refusal}"
    );

    // A foreign scope and a task outside the run are refused by name.
    assert!(matches!(
        cases.verify_completion("chunk:other", None).unwrap_err(),
        CompletionRefusal::ForeignScope { .. }
    ));
    assert!(matches!(
        cases
            .verify_completion("chunk:c1", Some("ghost"))
            .unwrap_err(),
        CompletionRefusal::TaskNotCurrent { .. }
    ));
}

#[test]
fn a_self_reported_pass_never_certifies_a_configured_gate() {
    let temp = tempfile::tempdir().unwrap();
    let world = world();
    let cases = stored(temp.path(), &world);
    planned(&cases);
    let mut claim = request("test", "cargo test");
    claim.command = None;
    claim.passed = Some(true);
    let admission = cases.admit_check(claim.clone()).unwrap();
    let execution = ShipUseCases::self_reported(&claim);
    let mut reported = outcome(execution, "cargo test");
    reported.command = None;
    cases.settle_check(admission, reported).unwrap();
    let refusal = cases.verify_completion("chunk:c1", None).unwrap_err();
    assert!(refusal.to_string().contains("self-reported"), "{refusal}");
}

#[test]
fn late_settlement_after_rollover_stays_under_its_receipt_and_a_reused_name_starts_over() {
    let temp = tempfile::tempdir().unwrap();
    let world = world();
    let cases = stored(temp.path(), &world);
    planned(&cases);
    let admission = cases.admit_check(request("test", "cargo test")).unwrap();
    let first_run = admission.target.run_id.clone();
    cases.complete_task("verify", vec![]).unwrap();
    cases.finalize(vec![], None).unwrap();
    planned(&cases);
    let settled = cases
        .settle_check(
            admission.clone(),
            outcome(CheckExecution::Exited { code: 0 }, "cargo test"),
        )
        .unwrap();
    let CheckSettlement::Stale { receipt, .. } = settled else {
        panic!("a superseded run must not collect the result");
    };
    assert_eq!(receipt.id, admission.receipt_id);
    assert_eq!(receipt.target.run_id, first_run);
    assert!(receipt.passed() && !receipt.is_applied());
    let ledger = cases.engine().lock().unwrap().receipts().clone();
    assert_eq!(ledger.for_run(&first_run).unwrap(), vec![*receipt]);

    // The new run's `verify` has no evidence of its own.
    let new_run = cases.admit_task(None).unwrap().run_id;
    assert!(ledger.for_run(&new_run).unwrap().is_empty());
    assert!(matches!(
        cases.verify_completion("chunk:c1", None).unwrap_err(),
        CompletionRefusal::NoReceipts { .. }
    ));
    let fresh = run(
        &cases,
        "test",
        "cargo test",
        CheckExecution::Exited { code: 0 },
    );
    assert_eq!(fresh.receipt().attempt, 1);
    assert!(cases.verify_completion("chunk:c1", None).is_ok());
}

#[test]
fn without_an_evidence_capability_admission_still_works_and_completion_refuses() {
    let temp = tempfile::tempdir().unwrap();
    let persistence = Persistence::new(&PersistenceConfig {
        enabled: true,
        data_dir: temp.path().into(),
    });
    let cases = ShipUseCases::new(Arc::new(Mutex::new(
        ShipEngine::new("project".into()).with_persistence(persistence),
    )));
    planned(&cases);
    let settled = run(
        &cases,
        "test",
        "cargo test",
        CheckExecution::Exited { code: 0 },
    );
    assert!(settled.receipt().source.is_none());
    assert!(settled.receipt().is_applied());
    assert!(matches!(
        cases.verify_completion("chunk:c1", None).unwrap_err(),
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
    assert!(
        error.message.contains("source observation refused"),
        "{error}"
    );
    let ledger = cases.engine().lock().unwrap().receipts().clone();
    let run_id = cases.admit_task(None).unwrap().run_id;
    assert!(ledger.for_run(&run_id).unwrap().is_empty());
}
