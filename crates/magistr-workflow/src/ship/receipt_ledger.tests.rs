use super::*;
use crate::ship::application::CheckRequest;
use crate::ship::domain::check::CheckType;

fn admit(ledger: &ReceiptLedger, run: &str, id: &str) -> CheckReceipt {
    let target = TaskAdmission {
        execution: None,
        project_id: "project".into(),
        run_id: run.into(),
        task_id: "verify".into(),
        task_instance: format!("{run}-task"),
    };
    ledger
        .admit(&target, "test", |attempt| {
            CheckReceipt::admitted(
                id.into(),
                target.clone(),
                attempt,
                &CheckRequest {
                    task_id: None,
                    check_type: CheckType::Test,
                    name: "test".into(),
                    passed: None,
                    details: "diagnostic output, never evidence".into(),
                    required: true,
                    command: Some("cargo test".into()),
                    report: None,
                },
                "acceptance".into(),
                None,
                "2026-09-17T00:00:00Z".into(),
            )
        })
        .unwrap()
        .value
}

#[test]
fn current_run_and_exact_id_reads_do_not_decode_unrelated_history() {
    let temp = tempfile::tempdir().unwrap();
    let ledger = ReceiptLedger::on_disk(temp.path());
    for n in 0..128 {
        admit(&ledger, &format!("old-{n}"), &format!("old-{n}"));
    }
    let current = admit(&ledger, "current", "current");
    // Poison an old payload after indexing: even a freshly opened reader
    // must not touch it when it asks for another run or an exact ID.
    fs::write(temp.path().join("old-0.json"), b"unreadable old history").unwrap();
    let reopened = ReceiptLedger::on_disk(temp.path());
    assert_eq!(reopened.for_run("current").unwrap(), vec![current.clone()]);
    assert_eq!(reopened.get("current").unwrap(), Some(current));
    assert_eq!(admit(&reopened, "current", "retry").attempt, 2);
    reopened.update("current", |_| Ok(())).unwrap();
}

fn settled(ledger: &ReceiptLedger, id: &str) -> CheckReceipt {
    ledger
        .update(id, |r| {
            r.outcome = Some(crate::ship::application::CheckOutcome {
                passed: true,
                details: String::new(),
                verified: true,
                command: r.command.clone(),
                exit_code: Some(0),
                execution: crate::ship::application::CheckExecution::Exited { code: 0 },
                report: None,
                results: None,
            });
            r.stage = ReceiptStage::Settled;
            r.recorded_at = Some("2026-09-17T00:01:00Z".into());
            r.duration_ms = Some(60_000);
            r.settlement =
                Some(crate::ship::domain::receipt_settlement::ReceiptSettlement::Applied);
            r.settled_at = Some("2026-09-17T00:01:00Z".into());
            Ok(())
        })
        .unwrap()
        .value
}

#[test]
fn compaction_keeps_current_and_unsettled_in_memory_and_on_disk() {
    let temp = tempfile::tempdir().unwrap();
    for ledger in [
        ReceiptLedger::in_memory(),
        ReceiptLedger::on_disk(temp.path()),
    ] {
        for n in 0..128 {
            let id = format!("old-{n}");
            admit(&ledger, &format!("run-{n}"), &id);
            settled(&ledger, &id);
        }
        let pending = admit(&ledger, "old-flight", "pending");
        let current = admit(&ledger, "current", "current");
        let passed = settled(&ledger, "current");
        let retry = admit(&ledger, "current", "retry");
        let report = ledger.compact("current", || Ok(BTreeSet::new())).unwrap();
        assert_eq!(
            report,
            ReceiptCompaction {
                storage: if ledger.directory().is_some() {
                    ReceiptStorage::File
                } else {
                    ReceiptStorage::Memory
                },
                removed: 128,
                retained_current: 2,
                retained_unsettled: 1,
                retained_referenced: 0,
            }
        );
        assert_eq!(ledger.get("pending").unwrap(), Some(pending.clone()));
        assert_eq!(ledger.for_run("current").unwrap(), vec![passed, retry]);
        assert!(ledger.get("old-0").unwrap().is_none());
        let late = settled(&ledger, "pending");
        assert_eq!(late.target, pending.target);
        assert_eq!(late.attempt, pending.attempt);
        assert_eq!(
            ledger
                .compact("current", || Ok(BTreeSet::new()))
                .unwrap()
                .removed,
            1
        );
        assert_eq!(
            ledger
                .compact("current", || Ok(BTreeSet::new()))
                .unwrap()
                .removed,
            0
        );
        assert_eq!(
            admit(&ledger, "current", "again").attempt,
            current.attempt + 2
        );
    }
}

#[test]
fn derived_index_restarts_partial_build_without_changing_receipts() {
    let temp = tempfile::tempdir().unwrap();
    let receipt = admit(&ReceiptLedger::in_memory(), "saved-run", "original");
    let bytes = serde_json::to_vec_pretty(&receipt).unwrap();
    fs::write(temp.path().join("original.json"), &bytes).unwrap();
    // Crash in index construction, before its completion marker.
    add_membership(temp.path(), &receipt).unwrap();
    let ledger = ReceiptLedger::on_disk(temp.path());
    assert_eq!(ledger.for_run("saved-run").unwrap(), vec![receipt.clone()]);
    assert_eq!(fs::read(temp.path().join("original.json")).unwrap(), bytes);
    assert_eq!(admit(&ledger, "saved-run", "next").attempt, 2);
    assert_eq!(settled(&ledger, "original").attempt, 1);
    assert!(
        run_directory(temp.path(), "saved-run")
            .join("original.ref")
            .exists()
    );
}

#[test]
fn interrupted_admission_and_diagnostics_are_never_evidence() {
    let temp = tempfile::tempdir().unwrap();
    let ledger = ReceiptLedger::on_disk(temp.path());
    assert!(ledger.for_run("current").unwrap().is_empty());
    let receipt = admit(&ReceiptLedger::in_memory(), "current", "unpublished");
    add_membership(temp.path(), &receipt).unwrap();
    fs::write(
        temp.path().join(".unpublished.json-123.tmp"),
        b"partial receipt",
    )
    .unwrap();
    fs::write(temp.path().join("diagnostics.log"), b"test passed").unwrap();
    assert!(ledger.for_run("current").unwrap().is_empty());
    assert!(ledger.get("unpublished").unwrap().is_none());
    let published = admit(&ledger, "current", "published");
    assert_eq!(published.attempt, 1);
    assert!(
        !serde_json::to_string(&published)
            .unwrap()
            .contains("diagnostic output")
    );
    assert_eq!(
        ledger
            .compact("current", || Ok(BTreeSet::new()))
            .unwrap()
            .retained_current,
        1
    );
    assert!(temp.path().join("diagnostics.log").exists());
}

#[test]
fn corruption_refuses_compaction_without_removing_any_receipt() {
    let temp = tempfile::tempdir().unwrap();
    let ledger = ReceiptLedger::on_disk(temp.path());
    admit(&ledger, "old", "old");
    settled(&ledger, "old");
    fs::write(temp.path().join("broken.json"), b"broken").unwrap();
    assert!(ledger.compact("current", || Ok(BTreeSet::new())).is_err());
    assert!(ledger.get("old").unwrap().is_some());
    assert!(ledger.get("../outside").is_err());
}

#[test]
fn concurrent_settlement_preserves_exact_admission_during_compaction() {
    let temp = tempfile::tempdir().unwrap();
    let ledger = ReceiptLedger::on_disk(temp.path());
    let pending = admit(&ledger, "old", "pending");
    let current = admit(&ledger, "current", "current");
    let barrier = Arc::new(std::sync::Barrier::new(2));
    std::thread::scope(|scope| {
        let other = ReceiptLedger::on_disk(temp.path());
        let worker_barrier = Arc::clone(&barrier);
        let worker = scope.spawn(move || {
            worker_barrier.wait();
            settled(&other, "pending")
        });
        barrier.wait();
        ledger.compact("current", || Ok(BTreeSet::new())).unwrap();
        let result = worker.join().unwrap();
        assert_eq!(result.target, pending.target);
        assert_eq!(result.attempt, pending.attempt);
    });
    // Either compaction ran first (late receipt is now retained), or it ran
    // after settlement (old receipt removed); the returned evidence is identical.
    assert_eq!(ledger.for_run("current").unwrap(), vec![current]);
}

#[test]
fn retained_references_are_sampled_under_lock_and_errors_delete_nothing() {
    let temp = tempfile::tempdir().unwrap();
    let ledger = ReceiptLedger::on_disk(temp.path());
    admit(&ledger, "old", "referenced");
    settled(&ledger, "referenced");
    let report = ledger
        .compact("current", || {
            let other = OpenOptions::new()
                .read(true)
                .write(true)
                .open(temp.path().join(".lock"))
                .unwrap();
            assert!(matches!(
                other.try_lock(),
                Err(std::fs::TryLockError::WouldBlock)
            ));
            Ok(BTreeSet::from(["referenced".into()]))
        })
        .unwrap();
    assert_eq!(report.retained_referenced, 1);
    assert!(
        ledger
            .compact("current", || Err("checkpoint unavailable".into()))
            .is_err()
    );
    assert!(ledger.get("referenced").unwrap().is_some());
}

#[test]
fn deletion_followed_by_index_failure_reports_uncertain_and_recovers() {
    let temp = tempfile::tempdir().unwrap();
    let ledger = ReceiptLedger::on_disk(temp.path());
    admit(&ledger, "old-a", "a");
    settled(&ledger, "a");
    admit(&ledger, "old-b", "b");
    settled(&ledger, "b");
    let member = run_directory(temp.path(), "old-a").join("a.ref");
    fs::remove_file(&member).unwrap();
    fs::create_dir(&member).unwrap();
    let error = ledger
        .compact("current", || Ok(BTreeSet::new()))
        .unwrap_err();
    assert_eq!(error.effect(), "unknown");
    assert!(ledger.get("a").unwrap().is_none());
    assert!(ledger.get("b").unwrap().is_some());
    let recovered = ledger.compact("current", || Ok(BTreeSet::new())).unwrap();
    assert_eq!(recovered.storage, ReceiptStorage::File);
    assert_eq!(recovered.removed, 1);
    assert!(ledger.get("b").unwrap().is_none());
}
