//! Crash injection: each test writes the intent a live loop would have
//! written, performs the effect or not, and drops the loop without settling
//! — exactly what a killed process leaves on disk — then recovers and asserts
//! what recovery establishes.

use super::*;
use crate::workflow::iteration::tests::{
    add_native_chunk, complete_native_chunk, native_engines, setup_project,
};
use crate::workflow::iteration::{IterationLoop, PreCheckResult};
use magistr_engine::native::ChunkStatus;
use magistr_engine::native::TaskType;
use std::path::Path;

fn recover(dir: &Path, spec: &Path, native: Option<NativeEngines>) -> IterationLoop {
    IterationLoop::open_or_recover(dir, spec, 5, native).expect("recovery")
}

fn native_loop(dir: &Path, spec: &Path, engines: &NativeEngines) -> IterationLoop {
    let mut il = IterationLoop::with_max(dir, spec, 5)
        .unwrap()
        .with_native(Some(engines.clone()));
    assert_eq!(il.pre_iteration_check().unwrap(), PreCheckResult::Continue);
    il
}

fn outcome_of(il: &IterationLoop, attempt: &AttemptId) -> OperationOutcome {
    il.operations().record(attempt).expect("recorded").outcome
}

fn commit(dir: &Path, message: &str) {
    let out = std::process::Command::new("git")
        .args(["commit", "--allow-empty", "-m", message])
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success());
}

#[test]
fn an_attempt_is_reserved_by_the_checkpoint_that_carries_it() {
    let (dir, spec) = setup_project("- [ ] item\n");
    let mut il = IterationLoop::with_max(dir.path(), &spec, 5).unwrap();
    assert!(il.operations().records().is_empty());
    il.pre_iteration_check().unwrap();
    let reserved = il.operations().records();
    assert_eq!(reserved.len(), 1);
    assert_eq!(reserved[0].kind, OperationKind::Reservation);
    assert_eq!(reserved[0].target, "attempt:0");
    assert_eq!(reserved[0].outcome, OperationOutcome::Completed);
    // A second pre-check of the same attempt reserves nothing new.
    il.pre_iteration_check().unwrap();
    assert_eq!(il.operations().records().len(), 1);
    il.persist().unwrap();

    // Crash mid-write: the temporary is there, the rename never happened.
    let torn = dir
        .path()
        .join(".magistr")
        .join(format!(".loop-state.json.tmp-{}", std::process::id()));
    std::fs::write(&torn, b"{\"iteration\": 1, \"run\": ").unwrap();
    drop(il);
    let recovered = recover(dir.path(), &spec, None);
    assert_eq!(recovered.operations().records().len(), 1);
    assert_eq!(
        recovered.operations().records()[0].outcome,
        OperationOutcome::Completed
    );
    assert!(recovered.reconciliation().is_empty());
    assert_eq!(recovered.iteration(), 0);
    recovered.persist().unwrap();
    assert!(!torn.exists());
}

#[test]
fn a_native_mutation_is_reconciled_against_the_store() {
    let (dir, spec) = setup_project("- [ ] item\n");
    let engines = native_engines(&dir);
    add_native_chunk(&engines, "c1", 100, &[]);
    add_native_chunk(&engines, "c2", 200, &[]);
    let mut il = native_loop(dir.path(), &spec, &engines);
    let applied = il
        .begin_operation(
            OperationKind::NativeMutation,
            "complete:c1",
            "complete_chunk:c1",
        )
        .unwrap()
        .unwrap();
    let lost = il
        .begin_operation(
            OperationKind::NativeMutation,
            "complete:c2",
            "complete_chunk:c2",
        )
        .unwrap()
        .unwrap();
    il.persist().unwrap();
    // The effect lands for c1 and the process dies before settling either.
    engines.roadmap_start_chunk("c1").unwrap();
    complete_native_chunk(&engines, "c1");
    drop(il);

    let mut recovered = recover(dir.path(), &spec, Some(engines.clone()));
    assert_eq!(
        outcome_of(&recovered, &applied),
        OperationOutcome::Completed
    );
    assert_eq!(outcome_of(&recovered, &lost), OperationOutcome::Failed);
    assert_eq!(recovered.reconciliation().settled.len(), 2);
    assert!(recovered.reconciliation().is_clean());
    assert_eq!(
        recovered.pre_iteration_check().unwrap(),
        PreCheckResult::Continue
    );
    // The applied one is established: nothing is redone. The lost one
    // admits a retry as the next attempt of the same command.
    assert_eq!(
        recovered
            .begin_operation(
                OperationKind::NativeMutation,
                "complete:c1",
                "complete_chunk:c1",
            )
            .unwrap(),
        None
    );
    let retry = recovered
        .begin_operation(
            OperationKind::NativeMutation,
            "complete:c2",
            "complete_chunk:c2",
        )
        .unwrap()
        .unwrap();
    assert_eq!(retry.command(), lost.command());
    assert_eq!(retry.number().get(), 2);
    assert_eq!(
        engines.roadmap_chunk("c2").unwrap().status,
        ChunkStatus::Pending
    );
}

#[test]
fn every_native_mutation_verb_is_read_back_from_the_chunk() {
    let (dir, spec) = setup_project("- [ ] item\n");
    let engines = native_engines(&dir);
    for id in ["started", "blocked", "reopened", "untouched"] {
        add_native_chunk(&engines, id, 100, &[]);
    }
    let mut il = native_loop(dir.path(), &spec, &engines);
    let mut begin = |key: &str, target: &str| {
        il.begin_operation(OperationKind::NativeMutation, key, target)
            .unwrap()
            .unwrap()
    };
    let started = begin("start", "start_chunk:started");
    let blocked = begin("block", "set_blocked_by:blocked");
    let reopened = begin("reopen", "set_status:reopened:in_progress");
    let untouched = begin("untouched", "start_chunk:untouched");
    let unknown = begin("unknown", "rename_chunk:untouched");
    il.persist().unwrap();
    engines.roadmap_start_chunk("started").unwrap();
    engines
        .roadmap_set_blocked_by(
            "blocked",
            magistr_engine::native::BlockerKind::AwaitingHuman,
            "why".into(),
            None,
        )
        .unwrap();
    engines.roadmap_start_chunk("reopened").unwrap();
    drop(il);

    let recovered = recover(dir.path(), &spec, Some(engines));
    assert_eq!(
        outcome_of(&recovered, &started),
        OperationOutcome::Completed
    );
    assert_eq!(
        outcome_of(&recovered, &blocked),
        OperationOutcome::Completed
    );
    assert_eq!(
        outcome_of(&recovered, &reopened),
        OperationOutcome::Completed
    );
    assert_eq!(outcome_of(&recovered, &untouched), OperationOutcome::Failed);
    // A verb recovery cannot read back stays open and blocks the next attempt.
    assert_eq!(
        outcome_of(&recovered, &unknown),
        OperationOutcome::Indeterminate
    );
    assert_eq!(recovered.reconciliation().open.len(), 1);
}

#[test]
fn a_receipt_is_reconciled_against_the_checks_on_its_task() {
    let (dir, spec) = setup_project("- [ ] item\n");
    let engines = native_engines(&dir);
    add_native_chunk(&engines, "c1", 100, &[]);
    engines
        .ship_set_objective("obj".into(), vec![], vec![], "chunk:c1".into())
        .unwrap();
    engines
        .ship_add_task("verify".into(), "verify".into(), TaskType::Review, None)
        .unwrap();
    engines.ship_start_task("verify").unwrap();
    let mut il = native_loop(dir.path(), &spec, &engines);
    let landed = il
        .begin_operation(OperationKind::Receipt, "gate:test", "gate:test@task:verify")
        .unwrap()
        .unwrap();
    let lost = il
        .begin_operation(OperationKind::Receipt, "gate:lint", "gate:lint@task:verify")
        .unwrap()
        .unwrap();
    il.persist().unwrap();
    engines
        .ship_record_gate(
            Some("verify"),
            "test",
            "true".into(),
            0,
            "ok".into(),
            true,
            None,
        )
        .unwrap();
    drop(il);

    let recovered = recover(dir.path(), &spec, Some(engines));
    assert_eq!(outcome_of(&recovered, &landed), OperationOutcome::Completed);
    assert_eq!(outcome_of(&recovered, &lost), OperationOutcome::Failed);
    assert!(recovered.reconciliation().is_clean());
}

#[test]
fn a_native_mutation_or_receipt_without_engines_stays_open() {
    let (dir, spec) = setup_project("- [ ] item\n");
    let mut il = IterationLoop::with_max(dir.path(), &spec, 5).unwrap();
    il.pre_iteration_check().unwrap();
    let mutation = il
        .begin_operation(
            OperationKind::NativeMutation,
            "complete:c1",
            "complete_chunk:c1",
        )
        .unwrap()
        .unwrap();
    il.persist().unwrap();
    drop(il);
    let mut recovered = recover(dir.path(), &spec, None);
    assert_eq!(
        outcome_of(&recovered, &mutation),
        OperationOutcome::Indeterminate
    );
    match recovered.pre_iteration_check().unwrap() {
        PreCheckResult::Unreconciled(open) => assert_eq!(open[0].attempt, mutation),
        other => panic!("expected an unreconciled stop, got {other:?}"),
    }
    let refused = recovered
        .begin_operation(
            OperationKind::NativeMutation,
            "complete:c1",
            "complete_chunk:c1",
        )
        .unwrap_err();
    assert!(refused.to_string().contains("reconcile"), "{refused}");
}

#[test]
fn a_publication_is_cancelled_because_surfaces_reseed() {
    let (dir, spec) = setup_project("- [ ] item\n");
    let mut il = IterationLoop::with_max(dir.path(), &spec, 5).unwrap();
    il.pre_iteration_check().unwrap();
    let frame = il
        .begin_operation(
            OperationKind::Publication,
            "frame:chunk_completed:c1",
            "frame:chunk_completed:c1",
        )
        .unwrap()
        .unwrap();
    il.persist().unwrap();
    drop(il);
    let mut recovered = recover(dir.path(), &spec, None);
    assert_eq!(outcome_of(&recovered, &frame), OperationOutcome::Cancelled);
    assert!(recovered.reconciliation().is_clean());
    assert_eq!(
        recovered.pre_iteration_check().unwrap(),
        PreCheckResult::Continue
    );
}

#[test]
fn a_provider_dispatch_waits_for_its_runtime_then_settles_by_the_git_baseline() {
    let (dir, spec) = setup_project("- [ ] item\n");
    let mut il = IterationLoop::with_max(dir.path(), &spec, 5).unwrap();
    il.pre_iteration_check().unwrap();
    let mut runtime = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("a stand-in provider runtime");
    let dispatch = il
        .begin_operation(OperationKind::ProviderDispatch, "dispatch", "provider:test")
        .unwrap()
        .unwrap();
    il.persist().unwrap();
    il.start_operation(&dispatch, Some(format!("pid:{}", runtime.id())))
        .unwrap();
    il.persist().unwrap();
    drop(il);

    // The runtime outlived the loop: nothing is established, no retry.
    let mut recovered = recover(dir.path(), &spec, None);
    assert_eq!(
        outcome_of(&recovered, &dispatch),
        OperationOutcome::Indeterminate
    );
    assert_eq!(recovered.reconciliation().open.len(), 1);
    match recovered.pre_iteration_check().unwrap() {
        PreCheckResult::Unreconciled(open) => {
            assert_eq!(open[0].attempt, dispatch);
            assert_eq!(
                open[0].external.as_deref(),
                Some(&*format!("pid:{}", runtime.id()))
            );
        }
        other => panic!("expected an unreconciled stop, got {other:?}"),
    }
    assert_eq!(recovered.run_state().terminal_reason, None);
    recovered.persist().unwrap();
    drop(recovered);

    // The runtime is gone and left no commit: the dispatch failed and the
    // attempt keeps its baseline for a retry under a new attempt number.
    runtime.kill().unwrap();
    runtime.wait().unwrap();
    let mut recovered = recover(dir.path(), &spec, None);
    assert_eq!(outcome_of(&recovered, &dispatch), OperationOutcome::Failed);
    assert_eq!(
        recovered.pre_iteration_check().unwrap(),
        PreCheckResult::Continue
    );
    let retry = recovered
        .begin_operation(OperationKind::ProviderDispatch, "dispatch", "provider:test")
        .unwrap()
        .unwrap();
    assert_eq!(retry.number().get(), 2);
    // Accepted, never launched: recovery still has nothing but the baseline.
    assert_eq!(recovered.consecutive_no_commits(), 0);
    recovered.persist().unwrap();
    drop(recovered);

    // A commit past the baseline while the loop was absent establishes
    // that the (now gone) runtime completed its work.
    commit(dir.path(), "feat: landed while the loop was down");
    let recovered = recover(dir.path(), &spec, None);
    assert_eq!(outcome_of(&recovered, &retry), OperationOutcome::Completed);
    assert!(recovered.reconciliation().is_clean());
}

#[test]
fn settled_records_stop_counters_and_budget_survive_the_atomic_checkpoint() {
    let (dir, spec) = setup_project("- [ ] item\n");
    let mut il = IterationLoop::with_max(dir.path(), &spec, 5).unwrap();
    il.pre_iteration_check().unwrap();
    let dispatch = il
        .begin_operation(OperationKind::ProviderDispatch, "dispatch", "provider:test")
        .unwrap()
        .unwrap();
    il.start_operation(&dispatch, Some("pid:0".into())).unwrap();
    il.settle_operation(&dispatch, OperationOutcome::Completed)
        .unwrap();
    il.observe_budget(1.25);
    il.post_iteration_check();
    il.persist().unwrap();
    let before = il.state().clone();
    drop(il);
    let recovered = recover(dir.path(), &spec, None);
    assert_eq!(recovered.state(), &before);
    assert_eq!(recovered.run_state().budget_total_usd, 1.25);
    assert_eq!(recovered.consecutive_no_commits(), 1);
    assert!(recovered.reconciliation().is_empty());
    // The run identity is what scopes every record; the reservation and
    // the dispatch both belong to it.
    for record in recovered.operations().records() {
        assert_eq!(record.attempt.command().run(), &recovered.state().run_id);
    }
    assert_eq!(
        recovered.state().run_id.project().as_str(),
        dir.path().file_name().unwrap().to_str().unwrap()
    );
}

#[test]
fn a_native_run_is_named_after_the_native_project() {
    let (dir, spec) = setup_project("- [ ] item\n");
    let engines = native_engines(&dir);
    add_native_chunk(&engines, "c1", 100, &[]);
    let il = native_loop(dir.path(), &spec, &engines);
    assert_eq!(il.state().run_id.project().as_str(), engines.project_id());
    assert!(il.state().run_id.local().starts_with("run-"));
}
