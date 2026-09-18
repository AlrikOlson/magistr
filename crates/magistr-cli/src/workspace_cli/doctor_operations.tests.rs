use std::path::Path;

use magistr_core::workflow::runtime_handle::RuntimeHandle;
use magistr_workspace::diagnostics::DoctorSeverity;
use serde_json::{Value, json};

use super::*;

fn record(
    run: &str,
    local: &str,
    kind: &str,
    target: &str,
    outcome: &str,
    external: Value,
) -> Value {
    json!({
        "attempt": { "command": { "run": { "project": "proj", "local": run }, "local": local }, "number": 1 },
        "kind": kind,
        "target": target,
        "outcome": outcome,
        "external": external,
    })
}

fn write_checkpoint(root: &Path, run: &str, records: Vec<Value>, terminal_reason: Value) {
    let state = root.join(".magistr");
    std::fs::create_dir_all(&state).expect("state dir");
    let checkpoint = json!({
        "iteration": 1,
        "max_iterations": 5,
        "run": {
            "mode": "chunk_loop",
            "completed_attempts": 1,
            "max_iterations": 5,
            "consecutive_no_commits": 0,
            "budget_total_usd": 0.0,
            "budget_limit_usd": null,
            "terminal_reason": terminal_reason,
            "last_settled_attempt": 0,
        },
        "run_id": { "project": "proj", "local": run },
        "operations": { "records": records },
        "attempt_started": 1,
        "native_chunk_id": "chunk-a",
        "last_commit_sha": "abc123",
        "last_status": "success",
        "spec_checked": 0,
        "spec_total": 1,
    });
    std::fs::write(
        state.join("loop-state.json"),
        serde_json::to_string_pretty(&checkpoint).expect("json"),
    )
    .expect("checkpoint");
}

/// A pid that named a process which has since exited and been reaped.
fn gone_pid() -> u32 {
    let mut child = std::process::Command::new("true")
        .spawn()
        .expect("spawn true");
    let pid = child.id();
    child.wait().expect("reap");
    pid
}

fn rows(section: &DoctorSection) -> Vec<Value> {
    section.detail["records"]
        .as_array()
        .cloned()
        .expect("records")
}

fn states(section: &DoctorSection) -> Vec<String> {
    rows(section)
        .iter()
        .map(|row| row["state"].as_str().expect("state").to_owned())
        .collect()
}

#[test]
fn a_pending_dispatch_is_recoverable_without_claiming_a_runtime() {
    let dir = tempfile::tempdir().unwrap();
    let run = format!("run-1-{}", gone_pid());
    let mut pending = record(
        &run,
        "0/dispatch",
        "provider_dispatch",
        "dispatch",
        "accepted",
        Value::Null,
    );
    pending["dispatch_phase"] = "pending".into();
    write_checkpoint(dir.path(), &run, vec![pending], Value::Null);
    let path = dir.path().join(".magistr/loop-state.json");
    let before = std::fs::read(&path).unwrap();
    let section = operations_section(dir.path(), None);
    assert_eq!(states(&section), ["recoverable"]);
    assert_eq!(rows(&section)[0]["dispatch_phase"], "pending");
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn no_checkpoint_is_an_info_finding_and_a_torn_one_is_unhealthy() {
    let dir = tempfile::tempdir().expect("tempdir");
    let section = operations_section(dir.path(), None);
    assert_eq!(section.id, DoctorSectionId::Operations);
    assert_eq!(section.findings[0].severity, DoctorSeverity::Info);
    assert!(section.findings[0].symptom.contains("no run checkpoint"));
    assert_eq!(section.detail["present"], false);
    assert!(
        !dir.path().join(".magistr").exists(),
        "the probe creates nothing"
    );

    std::fs::create_dir_all(dir.path().join(".magistr")).expect("state dir");
    std::fs::write(dir.path().join(".magistr/loop-state.json"), "{ torn").expect("torn");
    let section = operations_section(dir.path(), None);
    assert_eq!(section.findings[0].severity, DoctorSeverity::Unhealthy);
    assert!(section.findings[0].symptom.contains("cannot be read"));
    assert!(
        section.findings[0]
            .remediation
            .as_deref()
            .is_some_and(|r| r.contains("loop-state.json"))
    );
    assert_eq!(section.detail["readable"], false);
}

#[test]
fn a_lost_loop_explains_stalled_recoverable_settled_and_indeterminate_records() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut runtime = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("a runtime that outlives the loop");
    let handle = RuntimeHandle::observe(runtime.id()).expect("live handle");
    let run = format!("run-1-{}", gone_pid());
    write_checkpoint(
        dir.path(),
        &run,
        vec![
            record(
                &run,
                "0/attempt",
                "reservation",
                "attempt:0",
                "completed",
                Value::Null,
            ),
            record(
                &run,
                "0/dispatch",
                "provider_dispatch",
                "dispatch",
                "running",
                json!(handle.to_string()),
            ),
            record(
                &run,
                "0/gate:test",
                "receipt",
                "gate:test@task:verify",
                "running",
                json!("receipt:r-17"),
            ),
            record(
                &run,
                "0/publish",
                "publication",
                "chunk:chunk-a",
                "accepted",
                Value::Null,
            ),
            record(
                &run,
                "0/start",
                "native_mutation",
                "start_chunk:chunk-a",
                "indeterminate",
                Value::Null,
            ),
            record(
                &run,
                "0/dispatch-nohandle",
                "provider_dispatch",
                "dispatch",
                "accepted",
                Value::Null,
            ),
        ],
        Value::Null,
    );

    let section = operations_section(dir.path(), Some(3));
    assert_eq!(
        states(&section),
        [
            "settled",
            "stalled",
            "recoverable",
            "recoverable",
            "recoverable",
            "indeterminate"
        ]
    );
    assert_eq!(section.detail["project"], "proj");
    assert_eq!(section.detail["run"], run);
    assert_eq!(section.detail["workspace"]["generation"], 3);
    assert_eq!(section.detail["loopProcess"]["alive"], false);
    let listed = rows(&section);
    let dispatch = &listed[1];
    assert_eq!(dispatch["runtime"]["pid"], runtime.id());
    assert_eq!(dispatch["runtime"]["live"], true);
    assert_eq!(dispatch["settlement"]["ownershipReleased"], false);
    assert_eq!(dispatch["settlement"]["terminalObserved"], false);
    assert_eq!(dispatch["settlement"]["effectsSettled"], false);
    assert_eq!(dispatch["settlement"]["cancellationRequested"], false);
    assert!(
        dispatch["explanation"]
            .as_str()
            .expect("text")
            .contains("waits for it (unreconciled)"),
        "{dispatch}"
    );
    let receipt = &listed[2];
    assert_eq!(receipt["check"]["gate"], "test");
    assert_eq!(receipt["check"]["task"], "verify");
    assert_eq!(receipt["check"]["receipt"], "r-17");
    assert_eq!(receipt["settlement"]["ownershipReleased"], true);
    assert!(
        receipt["explanation"]
            .as_str()
            .expect("text")
            .contains("durable receipt")
    );
    let settled = &listed[0];
    assert_eq!(settled["settlement"]["terminalObserved"], true);
    assert_eq!(settled["settlement"]["effectsSettled"], true);
    assert_eq!(settled["attempt"]["command"], "0/attempt");
    assert_eq!(settled["attempt"]["number"], 1);
    let no_handle = &listed[5];
    assert_eq!(no_handle["runtime"], Value::Null);
    assert_eq!(no_handle["settlement"]["ownershipReleased"], Value::Null);
    assert!(
        no_handle["explanation"]
            .as_str()
            .expect("text")
            .contains("magistr resolve-dispatch")
    );

    let summary = &section.findings[0];
    assert_eq!(summary.severity, DoctorSeverity::Healthy);
    assert!(
        summary
            .symptom
            .contains("6 record(s), 1 settled, 0 live, 1 stalled, 3 recoverable, 1 indeterminate"),
        "{}",
        summary.symptom
    );
    assert!(summary.symptom.contains("gone"), "{}", summary.symptom);
    let stalled = section
        .findings
        .iter()
        .find(|f| f.symptom.starts_with("stalled:"))
        .expect("stalled finding");
    assert_eq!(stalled.severity, DoctorSeverity::Unhealthy);
    assert!(
        stalled
            .symptom
            .contains(&format!("runtime pid {}", runtime.id()))
    );
    assert!(
        stalled
            .remediation
            .as_deref()
            .is_some_and(|r| r.contains("wait for that runtime"))
    );
    assert!(
        section
            .findings
            .iter()
            .any(|f| f.symptom.starts_with("indeterminate:"))
    );
    assert_eq!(
        section
            .findings
            .iter()
            .filter(|f| f.symptom.starts_with("recoverable:"))
            .count(),
        3
    );

    // The runtime ends: the same record is now recoverable from Git.
    runtime.kill().expect("end the runtime");
    runtime.wait().expect("reap");
    let section = operations_section(dir.path(), Some(3));
    assert_eq!(states(&section)[1], "recoverable");
    let dispatch = &rows(&section)[1];
    assert_eq!(dispatch["runtime"]["live"], false);
    assert_eq!(dispatch["settlement"]["ownershipReleased"], true);
    assert!(
        dispatch["explanation"]
            .as_str()
            .expect("text")
            .contains("Git baseline")
    );
    assert!(
        !section
            .findings
            .iter()
            .any(|f| f.symptom.starts_with("stalled:"))
    );
    // The checkpoint was read, never marked or rewritten.
    let on_disk: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join(".magistr/loop-state.json")).expect("read"),
    )
    .expect("json");
    assert_eq!(on_disk["operations"]["records"][1]["outcome"], "running");
}

#[test]
fn a_running_loop_owns_its_unsettled_records_and_a_latched_stop_is_a_cancellation() {
    let dir = tempfile::tempdir().expect("tempdir");
    let run = format!("run-1-{}", std::process::id());
    write_checkpoint(
        dir.path(),
        &run,
        vec![
            record(
                &run,
                "0/dispatch",
                "provider_dispatch",
                "dispatch",
                "running",
                json!("pid:1"),
            ),
            record(
                &run,
                "0/start",
                "native_mutation",
                "start_chunk:chunk-a",
                "accepted",
                Value::Null,
            ),
        ],
        json!("cancelled"),
    );
    let section = operations_section(dir.path(), None);
    assert_eq!(states(&section), ["live", "live"]);
    assert_eq!(section.detail["loopProcess"]["pid"], std::process::id());
    assert_eq!(section.detail["loopProcess"]["alive"], true);
    for row in rows(&section) {
        assert_eq!(row["settlement"]["cancellationRequested"], true, "{row}");
    }
    assert!(
        section.findings[0]
            .symptom
            .contains("stop latched: cancelled")
    );
    assert!(section.findings[0].symptom.contains("pid evidence only"));
    assert!(
        !section
            .findings
            .iter()
            .any(|f| f.severity == DoctorSeverity::Unhealthy)
    );
    assert_eq!(
        section.detail["identities"]["session"]
            .as_str()
            .map(|s| s.starts_with("not persisted")),
        Some(true)
    );
}
