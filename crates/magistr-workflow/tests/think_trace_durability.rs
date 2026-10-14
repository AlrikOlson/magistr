//! Durability of the think trace under concurrent server processes and
//! restarts — regression suite for the 2026-06-09 incident where steps
//! recorded (and acked) by one server process were clobbered from disk by a
//! second, longer-lived process holding a stale in-memory history.
//!
//! The two `*_clobber*` / `*_outlives_trim*` tests reproduce the loss
//! mechanism and MUST fail on the pre-fix whole-file-overwrite persistence.

use std::collections::HashSet;

use magistr_workflow::think::config::ThinkConfig;
use magistr_workflow::think::domain::{NextAction, ThinkStep};
use magistr_workflow::think::engine::core::ReasoningServer;
use tempfile::TempDir;

fn quiet_config() -> ThinkConfig {
    let mut c = ThinkConfig::default();
    c.display.color_output = false;
    c
}

fn persisting_config(tmp: &TempDir) -> ThinkConfig {
    let mut c = quiet_config();
    c.persistence.enabled = true;
    c.persistence.data_dir = tmp.path().to_path_buf();
    c
}

fn step_n(n: u32) -> ThinkStep {
    ThinkStep {
        step_number: n,
        estimated_total: 100,
        purpose: format!("step {n}"),
        context: "durability test".into(),
        thought: format!("thought {n}"),
        outcome: format!("outcome {n}"),
        next_action: NextAction::Text("next".into()),
        rationale: "rationale".into(),
        confidence: None,
        uncertainty_notes: None,
        revises_step: None,
        revision_reason: None,
        revised_by: None,
        is_final_step: None,
        branch_from: None,
        branch_id: None,
        branch_name: None,
        tools_used: None,
        dependencies: None,
        timestamp: None,
        duration_ms: None,
        session_id: None,
        pinned: None,
        cwd: None,
        execution_ref: None,
        record_id: None,
    }
}

fn persisted_step_numbers(tmp: &TempDir) -> HashSet<u32> {
    // A fresh engine on the same data dir is "a restart": whatever it can see
    // is what actually survived on disk.
    let s = ReasoningServer::new(persisting_config(tmp));
    s.history().steps.iter().map(|st| st.step_number).collect()
}

/// THE INCIDENT, reduced: two live server processes share one data dir (and
/// one project). B records a step (acked + persisted). A — which loaded
/// before B's write and knows nothing of it — then records its own step.
/// A's save must not erase B's acked step from disk.
#[test]
fn concurrent_writer_does_not_clobber_acked_steps() {
    let tmp = TempDir::new().unwrap();

    let mut a = ReasoningServer::new(persisting_config(&tmp)); // loads (empty) state
    let mut b = ReasoningServer::new(persisting_config(&tmp)); // loads (empty) state

    let _ = b.process_step_native(step_n(1)); // B: record + persist step 1
    let _ = a.process_step_native(step_n(2)); // A: stale memory, record + persist step 2

    let survived = persisted_step_numbers(&tmp);
    assert!(
        survived.contains(&1),
        "step 1 was acked to B but a stale concurrent writer erased it from disk; survived: {survived:?}"
    );
    assert!(survived.contains(&2), "step 2 lost; survived: {survived:?}");
}

/// Same mechanism, interleaved both ways: every acked step from either
/// process must survive.
#[test]
fn interleaved_writers_union_on_disk() {
    let tmp = TempDir::new().unwrap();

    let mut a = ReasoningServer::new(persisting_config(&tmp));
    let mut b = ReasoningServer::new(persisting_config(&tmp));

    let _ = a.process_step_native(step_n(1));
    let _ = b.process_step_native(step_n(2));
    let _ = a.process_step_native(step_n(3));
    let _ = b.process_step_native(step_n(4));

    let survived = persisted_step_numbers(&tmp);
    for n in 1..=4 {
        assert!(
            survived.contains(&n),
            "step {n} lost; survived: {survived:?}"
        );
    }
}

/// The in-memory window is capped at `max_history_size`, but the DISK copy is
/// the durable record: steps that fall out of the window must still be on
/// disk after a restart (pre-fix, every save truncated the file to the
/// window).
#[test]
fn disk_archive_outlives_the_in_memory_trim_window() {
    let tmp = TempDir::new().unwrap();
    let mut cfg = persisting_config(&tmp);
    cfg.system.max_history_size = 3;

    let mut s = ReasoningServer::new(cfg);
    for n in 1..=5 {
        let _ = s.process_step_native(step_n(n));
    }
    assert_eq!(s.history().steps.len(), 3, "memory window should be capped");

    let survived = persisted_step_numbers(&tmp);
    for n in 1..=5 {
        assert!(
            survived.contains(&n),
            "step {n} fell out of the memory window AND off disk; survived: {survived:?}"
        );
    }
}

/// Restart durability (should already hold): record, drop, reload.
#[test]
fn steps_survive_a_process_restart() {
    let tmp = TempDir::new().unwrap();
    {
        let mut s = ReasoningServer::new(persisting_config(&tmp));
        let _ = s.process_step_native(step_n(1));
        let _ = s.process_step_native(step_n(2));
    }
    let survived = persisted_step_numbers(&tmp);
    assert!(survived.contains(&1) && survived.contains(&2));
}

/// Different projects sharing one data dir write different default files and
/// never see (or disturb) each other's steps — the cross-project half of the
/// incident (the surviving trace mixed tessera/kitbash/think-and-ship steps).
#[test]
fn projects_are_isolated_on_one_data_dir() {
    let tmp = TempDir::new().unwrap();
    let p1 = "proj-one-aaaaaa".to_string();
    let p2 = "proj-two-bbbbbb".to_string();

    let mut e1 = ReasoningServer::new_for_project(persisting_config(&tmp), p1.clone());
    let mut e2 = ReasoningServer::new_for_project(persisting_config(&tmp), p2.clone());
    let _ = e1.process_step_native(step_n(1));
    let _ = e2.process_step_native(step_n(1)); // same number in another project is fine

    let sessions = tmp.path().join("think").join("sessions");
    assert!(sessions.join(format!("{p1}.json")).exists());
    assert!(sessions.join(format!("{p2}.json")).exists());

    let r1 = ReasoningServer::new_for_project(persisting_config(&tmp), p1);
    assert_eq!(
        r1.history().steps.len(),
        1,
        "project one must reload exactly its own step"
    );
    assert_eq!(r1.history().steps[0].thought, "thought 1");
}

/// An explicit wipe must stay a wipe — the merge-on-save discipline must not
/// resurrect deleted steps from a pre-wipe file.
#[test]
fn wipe_is_not_resurrected_by_later_saves() {
    let tmp = TempDir::new().unwrap();
    let mut s = ReasoningServer::new(persisting_config(&tmp));
    let _ = s.process_step_native(step_n(1));
    let _ = s.process_step_native(step_n(2));

    s.clear_history().unwrap();
    let _ = s.process_step_native(step_n(10));

    let survived = persisted_step_numbers(&tmp);
    assert!(
        !survived.contains(&1) && !survived.contains(&2),
        "wiped steps resurrected: {survived:?}"
    );
    assert!(survived.contains(&10));
}

#[test]
fn an_unsupported_project_document_is_not_overwritten() {
    let directory = TempDir::new().unwrap();
    let config = persisting_config(&directory);
    let mut owner = ReasoningServer::new_for_project(config.clone(), "project".into());
    owner.process_step_native(step_n(1)).unwrap();
    let persistence = magistr_workflow::think::persistence::Persistence::for_project(
        &config.persistence,
        "project",
    );
    let path = persistence.store_path().unwrap();
    let mut document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    document["schema_version"] = serde_json::json!(999);
    let refused = serde_json::to_vec(&document).unwrap();
    std::fs::write(&path, &refused).unwrap();
    let before = owner.history_snapshot();
    let error = owner.process_step_native(step_n(2)).unwrap_err();
    assert_eq!(error.effect(), "none");
    assert_eq!(owner.history_snapshot(), before);
    assert_eq!(std::fs::read(&path).unwrap(), refused);
}

#[test]
fn concurrent_owners_allocate_numbers_for_distinct_identical_inputs() {
    use std::sync::{Arc, Barrier};
    let directory = TempDir::new().unwrap();
    let mut config = persisting_config(&directory);
    config.features.enable_sessions = true;
    let barrier = Arc::new(Barrier::new(2));
    let threads: Vec<_> = ["review", "build"]
        .into_iter()
        .map(|session| {
            let barrier = barrier.clone();
            let config = config.clone();
            std::thread::spawn(move || {
                let mut owner = ReasoningServer::new_for_project(config, "concurrent".into());
                let mut step = step_n(1);
                step.session_id = Some(session.into());
                barrier.wait();
                owner.process_step_native(step).unwrap()
            })
        })
        .collect();
    let results: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    assert_ne!(results[0].step.step_number, results[1].step.step_number);
    assert_ne!(results[0].step.record_id, results[1].step.record_id);
    let reopened = ReasoningServer::new_for_project(config, "concurrent".into());
    let saved: Vec<_> = reopened
        .sessions()
        .values()
        .flat_map(|entry| &entry.history.steps)
        .collect();
    assert_eq!(saved.len(), 2);
    for recorded in results {
        assert!(
            saved
                .iter()
                .any(|step| step.record_id == recorded.step.record_id)
        );
        assert!(matches!(
            recorded.source,
            magistr_workflow_contracts::ReadSource::File(_)
        ));
    }
}

#[test]
fn failed_record_pin_estimate_branch_and_wipe_keep_the_committed_state() {
    use magistr_workflow_contracts::ReadState;
    let directory = TempDir::new().unwrap();
    let config = persisting_config(&directory);
    let mut owner = ReasoningServer::new_for_project(config.clone(), "failure".into());
    owner.process_step_native(step_n(1)).unwrap();
    let mut branch = step_n(2);
    branch.branch_from = Some(1);
    branch.branch_name = Some("Check the evidence".into());
    let branch = owner
        .process_step_native(branch)
        .unwrap()
        .step
        .branch_id
        .unwrap();
    let persistence = magistr_workflow::think::persistence::Persistence::for_project(
        &config.persistence,
        "failure",
    );
    let path = persistence.store_path().unwrap();
    let saved = std::fs::read(&path).unwrap();
    let before = owner.history_snapshot();
    let lock = path.with_extension("json.lock");
    std::fs::remove_file(&lock).unwrap();
    std::fs::create_dir(&lock).unwrap();
    let errors = [
        owner.process_step_native(step_n(3)).unwrap_err(),
        owner.pin_step(1, true).unwrap_err(),
        owner.revise_estimate(30).unwrap_err(),
        owner
            .set_branch_status(&branch, "merged", Some(2))
            .unwrap_err(),
        owner.clear_history().unwrap_err(),
    ];
    assert!(
        errors
            .iter()
            .all(|error| error.effect() == "none" && error.store.is_some())
    );
    assert!(matches!(owner.mutation_state(), ReadState::Stale { .. }));
    assert_eq!(owner.history_snapshot(), before);
    assert_eq!(std::fs::read(&path).unwrap(), saved);
    std::fs::remove_dir(&lock).unwrap();
    owner.pin_step(1, true).unwrap();
    let retry = owner.process_step_native(step_n(3)).unwrap();
    assert_eq!(retry.step.step_number, 3);
    let reopened = ReasoningServer::new_for_project(config, "failure".into());
    assert_eq!(reopened.history().steps.len(), 3);
    assert_eq!(reopened.history().steps[0].pinned, Some(true));
}

#[test]
fn a_stale_owner_after_clear_cannot_resurrect_any_session() {
    let directory = TempDir::new().unwrap();
    let mut config = persisting_config(&directory);
    config.features.enable_sessions = true;
    let mut first = ReasoningServer::new_for_project(config.clone(), "clear".into());
    for name in ["one", "two"] {
        let mut step = step_n(0);
        step.session_id = Some(name.into());
        first.process_step_native(step).unwrap();
    }
    let mut stale = ReasoningServer::new_for_project(config.clone(), "clear".into());
    first.clear_history().unwrap();
    let mut step = step_n(0);
    step.session_id = Some("three".into());
    stale.process_step_native(step).unwrap();
    let reopened = ReasoningServer::new_for_project(config, "clear".into());
    assert_eq!(reopened.sessions().len(), 1);
    assert_eq!(reopened.sessions()["clear__three"].history.steps.len(), 1);
    assert_eq!(
        reopened.sessions()["clear__three"].history.steps[0].step_number,
        1
    );
}
