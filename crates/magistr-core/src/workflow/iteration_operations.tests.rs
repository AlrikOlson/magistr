//! Crash injection: each test writes the intent a live loop would have
//! written, performs the effect or not, and drops the loop without settling
//! — exactly what a killed process leaves on disk — then recovers and asserts
//! what recovery establishes.

use super::*;
use crate::workflow::iteration::tests::{
    add_native_chunk, complete_native_chunk, native_engines, setup_project,
};
use crate::workflow::iteration::{IterationLoop, PreCheckResult};
use magistr_engine::NativeEngines;
use magistr_engine::native::ChunkStatus;
use magistr_engine::native::TaskType;
use std::path::Path;

fn recover(dir: &Path, spec: &Path, native: Option<NativeEngines>) -> IterationLoop {
    IterationLoop::open_or_recover(dir, spec, 5, native.as_ref().map(NativeEngines::reader))
        .expect("recovery")
}

fn native_loop(dir: &Path, spec: &Path, engines: &NativeEngines) -> IterationLoop {
    let mut il = IterationLoop::with_max(dir, spec, 5)
        .unwrap()
        .with_native(Some(engines.reader()));
    assert_eq!(il.pre_iteration_check().unwrap(), PreCheckResult::Continue);
    il
}

fn outcome_of(il: &IterationLoop, attempt: &AttemptId) -> OperationOutcome {
    il.operations().record(attempt).expect("recorded").outcome
}

#[test]
fn an_accepted_dispatch_that_never_connected_recovers_failed_and_retries() {
    let (dir, spec) = setup_project("- [ ] item\n");
    let mut il = IterationLoop::with_max(dir.path(), &spec, 5).unwrap();
    assert_eq!(il.pre_iteration_check().unwrap(), PreCheckResult::Continue);
    let dispatch = il
        .begin_operation(OperationKind::ProviderDispatch, "dispatch", "dispatch")
        .unwrap()
        .unwrap();
    il.persist().unwrap();
    drop(il);

    let mut recovered = recover(dir.path(), &spec, None);
    assert_eq!(outcome_of(&recovered, &dispatch), OperationOutcome::Failed);
    assert!(recovered.reconciliation().open.is_empty());
    let retry = recovered
        .begin_operation(OperationKind::ProviderDispatch, "dispatch", "dispatch")
        .unwrap()
        .unwrap();
    assert_eq!(retry.command(), dispatch.command());
    assert_eq!(retry.number().get(), 2);
}

#[test]
fn a_legacy_accepted_dispatch_remains_unknown_across_repeated_recovery() {
    let (dir, spec) = setup_project("- [ ] item\n");
    let mut il = IterationLoop::with_max(dir.path(), &spec, 5).unwrap();
    il.pre_iteration_check().unwrap();
    let dispatch = il
        .begin_operation(OperationKind::ProviderDispatch, "dispatch", "dispatch")
        .unwrap()
        .unwrap();
    il.persist().unwrap();
    let checkpoint = dir.path().join(".magistr/loop-state.json");
    let mut json = serde_json::to_value(il.state()).unwrap();
    for record in json["operations"]["records"].as_array_mut().unwrap() {
        record.as_object_mut().unwrap().remove("dispatch_phase");
    }
    std::fs::write(&checkpoint, serde_json::to_vec(&json).unwrap()).unwrap();
    drop(il);
    for _ in 0..2 {
        let recovered = recover(dir.path(), &spec, None);
        assert_eq!(
            outcome_of(&recovered, &dispatch),
            OperationOutcome::Indeterminate
        );
        recovered.persist().unwrap();
    }
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
    let torn = dir.path().join(".magistr").join(format!(
        ".loop-state.json.tmp-{}-{}",
        std::process::id(),
        u64::MAX
    ));
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
    // A later replacement never consumes or removes another write's temporary.
    assert_eq!(
        std::fs::read(&torn).unwrap(),
        b"{\"iteration\": 1, \"run\": "
    );
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
        engines.roadmap_chunk("c2").unwrap().unwrap().status,
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

/// Engines with an objective and an active `verify` task — the shape the
/// loop's gates are recorded into.
fn engines_with_verify_task(dir: &tempfile::TempDir) -> NativeEngines {
    let engines = native_engines(dir);
    add_native_chunk(&engines, "c1", 100, &[]);
    engines
        .ship_set_objective("obj".into(), vec![], vec![], "chunk:c1".into())
        .unwrap();
    engines
        .ship_add_task("verify".into(), "verify".into(), TaskType::Review, None)
        .unwrap();
    engines.ship_start_task("verify").unwrap();
    engines
}

/// A receipt intended, admitted and started the way the loop does it:
/// the record carries the durable receipt id, and the admission is what
/// settlement needs.
fn admit_receipt(
    il: &mut IterationLoop,
    engines: &NativeEngines,
    gate: &str,
) -> (AttemptId, magistr_engine::native::CheckAdmission) {
    let target = format!("gate:{gate}@task:verify");
    let attempt = il
        .begin_operation(OperationKind::Receipt, &target, &target)
        .unwrap()
        .unwrap();
    let admission = engines
        .ship_admit_gate(Some("verify"), gate, "true".into(), true)
        .unwrap()
        .value;
    il.start_operation(&attempt, Some(format!("receipt:{}", admission.receipt_id)))
        .unwrap();
    (attempt, admission)
}

#[test]
fn compaction_keeps_a_settled_receipt_until_its_operation_is_reconciled() {
    use magistr_engine::native::{CheckExecution, NativeConfig, ProjectAuthority, WriterRole};
    let (dir, spec) = setup_project("- [ ] item\n");
    let inputs = crate::config::inputs::ProjectInputs::from_lookup(dir.path(), |_| None);
    let config = NativeConfig::from_lookup(dir.path(), |_| None)
        .with_data_dir(dir.path().join("store"))
        .with_evidence(std::sync::Arc::new(
            crate::evidence::WorkspaceEvidence::from_inputs(&inputs),
        ));
    let engines =
        ProjectAuthority::admit_for_project(&config, "retention".into(), WriterRole::Host)
            .unwrap()
            .open();
    engines
        .ship_set_objective("Old".into(), vec![], vec![], String::new())
        .unwrap();
    engines
        .ship_add_task("verify".into(), "Verify".into(), TaskType::Test, None)
        .unwrap();
    engines.ship_start_task("verify").unwrap();
    let mut il = IterationLoop::with_max(dir.path(), &spec, 5).unwrap();
    il.pre_iteration_check().unwrap();
    let (operation, admission) = admit_receipt(&mut il, &engines, "test");
    il.persist().unwrap();
    let id = admission.receipt_id.clone();
    engines
        .ship_settle_gate(
            admission,
            CheckExecution::Exited { code: 0 },
            String::new(),
            None,
        )
        .unwrap();
    engines.ship_complete_task("verify", vec![]).unwrap();
    engines.ship_finalize(vec![], None).unwrap();
    engines
        .ship_set_objective("New".into(), vec![], vec![], String::new())
        .unwrap();
    drop(il); // Crash before acknowledging the applied receipt in the checkpoint.
    assert_eq!(
        engines.ship_compact_receipts().unwrap().retained_referenced,
        1
    );
    assert!(
        engines.ship_receipt(&id).unwrap().is_some(),
        "unresolved operation must retain its authority"
    );
    let recovered = recover(dir.path(), &spec, Some(engines.clone()));
    assert_eq!(
        outcome_of(&recovered, &operation),
        OperationOutcome::Completed
    );
    recovered.persist().unwrap();
    assert_eq!(engines.ship_compact_receipts().unwrap().removed, 1);
    assert!(engines.ship_receipt(&id).unwrap().is_none());
}

#[test]
fn a_receipt_is_reconciled_against_its_own_receipt_never_a_same_named_check() {
    use magistr_engine::native::CheckExecution;
    let (dir, spec) = setup_project("- [ ] item\n");
    let engines = engines_with_verify_task(&dir);
    // The agent's own gate run on the same task, before the loop's set: a
    // same-named check that is not the loop's receipt.
    engines
        .ship_record_gate(
            Some("verify"),
            "test",
            "true".into(),
            0,
            "the agent's own run".into(),
            true,
            None,
        )
        .unwrap();
    let mut il = native_loop(dir.path(), &spec, &engines);
    let (lost, _) = admit_receipt(&mut il, &engines, "test");
    let (landed, landed_admission) = admit_receipt(&mut il, &engines, "lint");
    il.persist().unwrap();
    engines
        .ship_settle_gate(
            landed_admission,
            CheckExecution::Exited { code: 0 },
            "ok".into(),
            None,
        )
        .unwrap();
    drop(il);

    let recovered = recover(dir.path(), &spec, Some(engines));
    // The lost receipt was admitted and never settled: nothing was applied,
    // whatever same-named check sits on the task.
    assert_eq!(outcome_of(&recovered, &lost), OperationOutcome::Failed);
    assert_eq!(outcome_of(&recovered, &landed), OperationOutcome::Completed);
    assert!(recovered.reconciliation().is_clean());
}

#[test]
fn a_receipt_settled_stale_or_never_admitted_is_failed_and_a_foreign_one_stays_open() {
    use magistr_engine::native::CheckExecution;
    let (dir, spec) = setup_project("- [ ] item\n");
    let engines = engines_with_verify_task(&dir);
    let mut il = native_loop(dir.path(), &spec, &engines);
    let (stale, stale_admission) = admit_receipt(&mut il, &engines, "test");
    // A record that names another receipt than its own: the lint record
    // wearing the test receipt's id.
    let foreign = il
        .begin_operation(
            OperationKind::Receipt,
            "gate:lint@task:verify",
            "gate:lint@task:verify",
        )
        .unwrap()
        .unwrap();
    il.start_operation(
        &foreign,
        Some(format!("receipt:{}", stale_admission.receipt_id)),
    )
    .unwrap();
    let unadmitted = il
        .begin_operation(
            OperationKind::Receipt,
            "gate:check@task:verify",
            "gate:check@task:verify",
        )
        .unwrap()
        .unwrap();
    il.persist().unwrap();
    // The objective rolls over under the admitted test receipt: its
    // settlement is kept stale, never applied to the new run's task.
    engines.ship_finalize(vec![], None).unwrap();
    engines
        .ship_set_objective("next".into(), vec![], vec![], "chunk:c1".into())
        .unwrap();
    let settled = engines
        .ship_settle_gate(
            stale_admission,
            CheckExecution::Exited { code: 0 },
            "late".into(),
            None,
        )
        .unwrap();
    assert!(matches!(
        settled,
        magistr_engine::native::CheckSettlement::Stale { .. }
    ));
    drop(il);

    let recovered = recover(dir.path(), &spec, Some(engines));
    assert_eq!(outcome_of(&recovered, &stale), OperationOutcome::Failed);
    assert_eq!(
        outcome_of(&recovered, &unadmitted),
        OperationOutcome::Failed
    );
    assert_eq!(
        outcome_of(&recovered, &foreign),
        OperationOutcome::Indeterminate
    );
    assert_eq!(recovered.reconciliation().open.len(), 1);
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

/// A stand-in provider runtime the loop can be handed a handle for.
fn runtime() -> std::process::Child {
    std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("a stand-in provider runtime")
}

/// The handle of a runtime that has already exited — a launch the loop
/// recorded, and a process that is gone by the time recovery asks.
fn gone_runtime_handle() -> String {
    let mut child = runtime();
    let handle = RuntimeHandle::observe(child.id()).expect("a live child");
    child.kill().unwrap();
    child.wait().unwrap();
    handle.to_string()
}

fn git(dir: &Path, args: &[&str]) {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}");
}

#[test]
fn a_provider_dispatch_waits_for_its_runtime_then_settles_by_the_git_baseline() {
    let (dir, spec) = setup_project("- [ ] item\n");
    let mut il = IterationLoop::with_max(dir.path(), &spec, 5).unwrap();
    il.pre_iteration_check().unwrap();
    let mut runtime = runtime();
    let handle = RuntimeHandle::observe(runtime.id()).expect("a live runtime");
    let dispatch = il
        .begin_operation(OperationKind::ProviderDispatch, "dispatch", "provider:test")
        .unwrap()
        .unwrap();
    il.checkpoint_dispatch_launch(&dispatch).unwrap();
    il.start_operation(&dispatch, Some(handle.to_string()))
        .unwrap();
    il.persist().unwrap();
    drop(il);

    // The runtime outlived the loop: nothing is established, no retry —
    // not even once a commit lands meanwhile, since a live runtime is
    // nobody's to judge.
    commit(dir.path(), "chore: someone committed while the runtime ran");
    let mut recovered = recover(dir.path(), &spec, None);
    assert_eq!(
        outcome_of(&recovered, &dispatch),
        OperationOutcome::Indeterminate
    );
    assert_eq!(recovered.reconciliation().open.len(), 1);
    match recovered.pre_iteration_check().unwrap() {
        PreCheckResult::Unreconciled(open) => {
            assert_eq!(open[0].attempt, dispatch);
            assert_eq!(open[0].external.as_deref(), Some(&*handle.to_string()));
        }
        other => panic!("expected an unreconciled stop, got {other:?}"),
    }
    assert_eq!(recovered.run_state().terminal_reason, None);
    recovered.persist().unwrap();
    drop(recovered);

    // The runtime is gone; HEAD advanced from the attempt's baseline while
    // it lived: that is the runtime's work, done.
    runtime.kill().unwrap();
    runtime.wait().unwrap();
    let mut recovered = recover(dir.path(), &spec, None);
    assert_eq!(
        outcome_of(&recovered, &dispatch),
        OperationOutcome::Completed
    );
    assert!(recovered.reconciliation().is_clean());
    assert_eq!(
        recovered.pre_iteration_check().unwrap(),
        PreCheckResult::Continue
    );
    // Established: not launched again this attempt.
    assert_eq!(
        recovered
            .begin_operation(OperationKind::ProviderDispatch, "dispatch", "provider:test")
            .unwrap(),
        None
    );
    recovered.post_iteration_check();
    recovered.advance();
    recovered.persist().unwrap();

    // The next attempt: a launched runtime that died leaving no commit is
    // a failed dispatch, retried as attempt two of the same command.
    assert_eq!(
        recovered.pre_iteration_check().unwrap(),
        PreCheckResult::Continue
    );
    let dispatch = recovered
        .begin_operation(OperationKind::ProviderDispatch, "dispatch", "provider:test")
        .unwrap()
        .unwrap();
    recovered
        .start_operation(&dispatch, Some(gone_runtime_handle()))
        .unwrap();
    recovered.persist().unwrap();
    drop(recovered);
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
    assert_eq!(retry.command(), dispatch.command());
    assert_eq!(retry.number().get(), 2);
    assert_eq!(recovered.consecutive_no_commits(), 0);
}

#[test]
fn a_dispatch_without_a_runtime_handle_stays_open_whatever_head_did() {
    let (dir, spec) = setup_project("- [ ] item\n");
    let mut il = IterationLoop::with_max(dir.path(), &spec, 5).unwrap();
    il.pre_iteration_check().unwrap();
    // The pre-spawn marker landed, but the handle did not. A commit by
    // another process cannot establish what happened to this dispatch.
    let dispatch = il
        .begin_operation(OperationKind::ProviderDispatch, "dispatch", "provider:test")
        .unwrap()
        .unwrap();
    il.checkpoint_dispatch_launch(&dispatch).unwrap();
    drop(il);
    commit(dir.path(), "feat: a commit from nobody the loop knows");
    let mut recovered = recover(dir.path(), &spec, None);
    assert_eq!(
        outcome_of(&recovered, &dispatch),
        OperationOutcome::Indeterminate
    );
    match recovered.pre_iteration_check().unwrap() {
        PreCheckResult::Unreconciled(open) => {
            assert_eq!(open[0].attempt, dispatch);
            assert_eq!(open[0].external, None);
        }
        other => panic!("expected an unreconciled stop, got {other:?}"),
    }
    // A running record with no handle reads the same way.
    let mut il = IterationLoop::with_max(dir.path(), &spec, 5).unwrap();
    il.pre_iteration_check().unwrap();
    let dispatch = il
        .begin_operation(OperationKind::ProviderDispatch, "dispatch", "provider:test")
        .unwrap()
        .unwrap();
    il.start_operation(&dispatch, None).unwrap();
    il.persist().unwrap();
    drop(il);
    let recovered = recover(dir.path(), &spec, None);
    assert_eq!(
        outcome_of(&recovered, &dispatch),
        OperationOutcome::Indeterminate
    );
}

#[test]
fn a_reused_pid_is_not_the_runtime_and_a_dead_one_is_judged_only_by_an_advance() {
    let (dir, spec) = setup_project("- [ ] item\n");
    commit(dir.path(), "feat: the commit the attempt starts on");
    let mut il = IterationLoop::with_max(dir.path(), &spec, 5).unwrap();
    il.pre_iteration_check().unwrap();
    let live = runtime();
    let live_handle = RuntimeHandle::observe(live.id()).expect("a live runtime");
    // A handle naming the live process's id under another start identity:
    // the id was reused by something that is not this dispatch's runtime.
    let reused = RuntimeHandle::new(live.id(), live_handle.start().map(|start| start ^ 1));
    let dispatch = il
        .begin_operation(OperationKind::ProviderDispatch, "dispatch", "provider:test")
        .unwrap()
        .unwrap();
    il.start_operation(&dispatch, Some(reused.to_string()))
        .unwrap();
    il.persist().unwrap();
    drop(il);
    // HEAD moves, but not forward from the baseline: back to the initial
    // commit. Movement that is not an advance from the baseline is not
    // the runtime's commit.
    git(dir.path(), &["checkout", "--detach", "HEAD~1"]);
    let mut recovered = recover(dir.path(), &spec, None);
    let expected = if live_handle.start().is_some() {
        OperationOutcome::Failed
    } else {
        // A host that cannot tell processes apart cannot call it gone.
        OperationOutcome::Indeterminate
    };
    assert_eq!(outcome_of(&recovered, &dispatch), expected);
    let mut live = live;
    live.kill().unwrap();
    live.wait().unwrap();
    if expected == OperationOutcome::Failed {
        assert_eq!(
            recovered.pre_iteration_check().unwrap(),
            PreCheckResult::Continue
        );
    }
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

#[test]
fn unreadable_native_outcomes_remain_indeterminate_until_the_store_recovers() {
    let (dir, spec) = setup_project("- [ ] item\n");
    let engines = native_engines(&dir);
    add_native_chunk(&engines, "work", 1, &[]);
    let mut run = native_loop(dir.path(), &spec, &engines);
    let attempt = run
        .begin_operation(
            OperationKind::NativeMutation,
            "start:work",
            "start_chunk:work",
        )
        .unwrap()
        .unwrap();
    run.start_operation(&attempt, None).unwrap();
    run.persist().unwrap();
    engines.roadmap_start_chunk("work").unwrap();
    let path = dir
        .path()
        .join("tas-data/roadmap/sessions/iteration-native.json");
    let valid = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"{ unreadable roadmap").unwrap();
    drop(run);
    for _ in 0..2 {
        let mut recovered = recover(dir.path(), &spec, Some(engines.clone()));
        assert_eq!(
            outcome_of(&recovered, &attempt),
            OperationOutcome::Indeterminate
        );
        assert_eq!(recovered.reconciliation().open.len(), 1);
        assert!(matches!(
            recovered.pre_iteration_check().unwrap(),
            PreCheckResult::Unreconciled(_)
        ));
        recovered.persist().unwrap();
    }
    std::fs::write(path, valid).unwrap();
    let recovered = recover(dir.path(), &spec, Some(engines));
    assert_eq!(
        outcome_of(&recovered, &attempt),
        OperationOutcome::Completed
    );
    assert!(recovered.reconciliation().open.is_empty());
}

#[test]
fn receipt_recovery_uses_its_ledger_when_the_execution_summary_is_unreadable() {
    use magistr_engine::native::CheckExecution;
    let (dir, spec) = setup_project("- [ ] item\n");
    let engines = engines_with_verify_task(&dir);
    let mut run = native_loop(dir.path(), &spec, &engines);
    let (attempt, admission) = admit_receipt(&mut run, &engines, "test");
    run.persist().unwrap();
    let receipt_id = admission.receipt_id.clone();
    engines
        .ship_settle_gate(
            admission,
            CheckExecution::Exited { code: 0 },
            "Passed".into(),
            None,
        )
        .unwrap();
    drop(run);

    let reader = engines.reader();
    let config = reader.config();
    let path = config
        .persistence_config()
        .data_dir
        .join("ship")
        .join("executions")
        .join(config.execution_context().key())
        .join("sessions")
        .join(format!("{}.json", reader.project_id()));
    std::fs::write(&path, b"{ unreadable plan").unwrap();
    assert!(reader.ship_snapshot().is_err());
    assert!(reader.ship_receipt(&receipt_id).unwrap().is_some());

    let recovered = recover(dir.path(), &spec, Some(engines));
    assert_eq!(
        outcome_of(&recovered, &attempt),
        OperationOutcome::Completed
    );
    assert!(recovered.reconciliation().is_clean());
    assert_eq!(std::fs::read(path).unwrap(), b"{ unreadable plan");
}
