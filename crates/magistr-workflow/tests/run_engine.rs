use magistr_workflow::run::domain::artifact::{Artifact, ArtifactType};
use magistr_workflow::run::domain::check::CheckType;
use magistr_workflow::run::domain::goal::GoalStatus;
use magistr_workflow::run::domain::progress::ProgressType;
use magistr_workflow::run::domain::step::{StepStatus, StepType};
use magistr_workflow::run::engine::RunEngine;

fn engine() -> RunEngine {
    RunEngine::new("test-project-abc123".to_string())
}

// ── Goal ──────────────────────────────────────────────────────

#[test]
fn set_goal() {
    let mut e = engine();
    e.set_goal(
        "Build auth".into(),
        vec!["JWT works".into()],
        vec!["Validate every protected request".into()],
        "src/auth/".into(),
    )
    .unwrap();
    let obj = e.goal.as_ref().unwrap();
    assert_eq!(obj.description, "Build auth");
    assert_eq!(obj.acceptance_criteria, vec!["JWT works"]);
    assert_eq!(obj.status, GoalStatus::Open);
    assert!(obj.created_at.is_some());
}

#[test]
fn set_goal_overwrites() {
    let mut e = engine();
    e.set_goal("First".into(), vec![], vec![], String::new())
        .unwrap();
    e.set_goal("Second".into(), vec![], vec![], String::new())
        .unwrap();
    assert_eq!(e.goal.as_ref().unwrap().description, "Second");
}

#[test]
fn overwriting_an_in_flight_goal_preserves_steps() {
    let mut e = engine();
    e.set_goal("First".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step("t1".into(), "T".into(), StepType::Implement, None, None)
        .unwrap();
    // Goal is still Active (not shipped) — steps are kept, nothing cleared.
    let cleared = e
        .set_goal("Second".into(), vec![], vec![], String::new())
        .unwrap()
        .value;
    assert_eq!(cleared, 0);
    assert_eq!(e.steps.len(), 1);
}

#[test]
fn a_new_goal_after_shipping_starts_a_clean_cycle() {
    let mut e = engine();
    e.set_goal("Cycle one".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step("explore".into(), "E".into(), StepType::Research, None, None)
        .unwrap();
    e.add_step("verify".into(), "V".into(), StepType::Review, None, None)
        .unwrap();
    serde_json::to_value(e.finalize(vec![], Some("done".into())).unwrap().value).unwrap(); // goal → Completed
    assert_eq!(e.goal.as_ref().unwrap().status, GoalStatus::Finished);

    // The next task's goal must NOT inherit the finished cycle's steps —
    // the bleed that let a real session pile explore/implement/verify onto a
    // completed goal and then collide on reused ids.
    let cleared = e
        .set_goal("Cycle two".into(), vec![], vec![], String::new())
        .unwrap()
        .value;
    assert_eq!(cleared, 2);
    assert!(e.steps.is_empty());
    // And the fresh goal can reuse the same step ids without collision.
    e.add_step(
        "explore".into(),
        "E2".into(),
        StepType::Research,
        None,
        None,
    )
    .unwrap();
    assert_eq!(e.steps.len(), 1);
}

// ── Plan ───────────────────────────────────────────────────────────

#[test]
fn add_step() {
    let mut e = engine();
    e.set_goal("Goal".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step(
        "t1".into(),
        "First step".into(),
        StepType::Implement,
        Some("small".into()),
        None,
    )
    .unwrap();
    assert_eq!(e.steps.len(), 1);
    assert_eq!(e.steps[0].id, "t1");
    assert_eq!(e.steps[0].status, StepStatus::Planned);
    assert_eq!(e.goal.as_ref().unwrap().status, GoalStatus::Active);
}

#[test]
fn add_step_with_decision_thread() {
    let mut e = engine();
    e.set_goal("Goal".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step(
        "t1".into(),
        "Step".into(),
        StepType::Implement,
        None,
        Some("alt-approach".into()),
    )
    .unwrap();
    assert_eq!(e.steps[0].decision_thread, Some("alt-approach".into()));
}

#[test]
fn remove_planned_step() {
    let mut e = engine();
    e.set_goal("Goal".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step("t1".into(), "Step".into(), StepType::Implement, None, None)
        .unwrap();
    e.remove_step("t1").unwrap();
    assert!(e.steps.is_empty());
}

#[test]
fn remove_active_step_fails() {
    let mut e = engine();
    e.set_goal("Goal".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step("t1".into(), "Step".into(), StepType::Implement, None, None)
        .unwrap();
    e.start_step("t1").unwrap();
    assert!(e.remove_step("t1").is_err());
}

#[test]
fn remove_nonexistent_step_fails() {
    let e = engine();
    assert!(e.steps.is_empty());
}

#[test]
fn reorder_step() {
    let mut e = engine();
    e.set_goal("Goal".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step("a".into(), "A".into(), StepType::Implement, None, None)
        .unwrap();
    e.add_step("b".into(), "B".into(), StepType::Test, None, None)
        .unwrap();
    e.add_step("c".into(), "C".into(), StepType::Review, None, None)
        .unwrap();
    e.reorder_step("c", Some("a")).unwrap();
    let ids: Vec<&str> = e.steps.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, vec!["a", "c", "b"]);
}

#[test]
fn reorder_to_front() {
    let mut e = engine();
    e.set_goal("Goal".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step("a".into(), "A".into(), StepType::Implement, None, None)
        .unwrap();
    e.add_step("b".into(), "B".into(), StepType::Test, None, None)
        .unwrap();
    e.reorder_step("b", None).unwrap();
    let ids: Vec<&str> = e.steps.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, vec!["b", "a"]);
}

// ── Step lifecycle ─────────────────────────────────────────────────

#[test]
fn start_step() {
    let mut e = engine();
    e.set_goal("Goal".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step("t1".into(), "Step".into(), StepType::Implement, None, None)
        .unwrap();
    let step = e.start_step("t1").unwrap().value;
    assert_eq!(step.status, StepStatus::Active);
    assert!(step.started_at.is_some());
}

#[test]
fn start_second_step_fails() {
    let mut e = engine();
    e.set_goal("Goal".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step("t1".into(), "A".into(), StepType::Implement, None, None)
        .unwrap();
    e.add_step("t2".into(), "B".into(), StepType::Test, None, None)
        .unwrap();
    e.start_step("t1").unwrap();
    assert!(e.start_step("t2").is_err());
}

#[test]
fn complete_step() {
    let mut e = engine();
    e.set_goal("Goal".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step("t1".into(), "Step".into(), StepType::Implement, None, None)
        .unwrap();
    e.start_step("t1").unwrap();
    let artifacts = vec![Artifact {
        artifact_type: ArtifactType::Commit,
        reference: "abc123".into(),
        description: "initial commit".into(),
    }];
    let step = e.complete_step("t1", artifacts).unwrap().value;
    assert_eq!(step.status, StepStatus::Done);
    assert!(step.completed_at.is_some());
    assert_eq!(step.artifacts.len(), 1);
}

#[test]
fn complete_planned_step_fails() {
    let mut e = engine();
    e.set_goal("Goal".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step("t1".into(), "Step".into(), StepType::Implement, None, None)
        .unwrap();
    assert!(e.complete_step("t1", vec![]).is_err());
}

#[test]
fn block_step() {
    let mut e = engine();
    e.set_goal("Goal".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step("t1".into(), "Step".into(), StepType::Implement, None, None)
        .unwrap();
    e.start_step("t1").unwrap();
    let step = e
        .block_step("t1", "waiting on API key".into())
        .unwrap()
        .value;
    assert_eq!(step.status, StepStatus::Waiting);
    assert_eq!(step.waiting_reason, Some("waiting on API key".into()));
}

#[test]
fn restart_blocked_step() {
    let mut e = engine();
    e.set_goal("Goal".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step("t1".into(), "Step".into(), StepType::Implement, None, None)
        .unwrap();
    e.start_step("t1").unwrap();
    e.block_step("t1", "blocked".into()).unwrap();
    let step = e.start_step("t1").unwrap().value;
    assert_eq!(step.status, StepStatus::Active);
    assert_eq!(step.waiting_reason, None);
}

#[test]
fn complete_blocked_step() {
    let mut e = engine();
    e.set_goal("Goal".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step("t1".into(), "Step".into(), StepType::Implement, None, None)
        .unwrap();
    e.start_step("t1").unwrap();
    e.block_step("t1", "blocked".into()).unwrap();
    let step = e.complete_step("t1", vec![]).unwrap().value;
    assert_eq!(step.status, StepStatus::Done);
}

// ── Record ─────────────────────────────────────────────────────────

#[test]
fn record_progress_on_active_step() {
    let mut e = engine();
    e.set_goal("Goal".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step("t1".into(), "Step".into(), StepType::Implement, None, None)
        .unwrap();
    e.start_step("t1").unwrap();
    let progress = e
        .record_progress(
            None,
            ProgressType::Code,
            "wrote auth.rs".into(),
            vec!["src/auth.rs".into()],
            vec!["Edit".into()],
            "compiles".into(),
            Some(42),
        )
        .unwrap()
        .value;
    assert_eq!(progress.id, 1);
    assert_eq!(progress.note, Some(42));
    assert_eq!(progress.step_id, "t1");
}

#[test]
fn record_progress_explicit_step_id() {
    let mut e = engine();
    e.set_goal("Goal".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step("t1".into(), "Step".into(), StepType::Implement, None, None)
        .unwrap();
    e.start_step("t1").unwrap();
    let progress = e
        .record_progress(
            Some("t1"),
            ProgressType::Test,
            "ran tests".into(),
            vec![],
            vec![],
            "pass".into(),
            None,
        )
        .unwrap()
        .value;
    assert_eq!(progress.progress_type, ProgressType::Test);
    assert_eq!(progress.note, None);
}

#[test]
fn record_progress_no_active_step_fails() {
    let mut e = engine();
    e.set_goal("Goal".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step("t1".into(), "Step".into(), StepType::Implement, None, None)
        .unwrap();
    assert!(
        e.record_progress(
            None,
            ProgressType::Code,
            "x".into(),
            vec![],
            vec![],
            "y".into(),
            None
        )
        .is_err()
    );
}

#[test]
fn progress_ids_are_monotonic() {
    let mut e = engine();
    e.set_goal("Goal".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step("t1".into(), "Step".into(), StepType::Implement, None, None)
        .unwrap();
    e.start_step("t1").unwrap();
    let a1 = e
        .record_progress(
            None,
            ProgressType::Code,
            "a".into(),
            vec![],
            vec![],
            "".into(),
            None,
        )
        .unwrap()
        .value;
    let id1 = a1.id;
    let a2 = e
        .record_progress(
            None,
            ProgressType::Code,
            "b".into(),
            vec![],
            vec![],
            "".into(),
            None,
        )
        .unwrap()
        .value;
    let id2 = a2.id;
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
}

// ── Check ──────────────────────────────────────────────────────────

#[test]
fn record_check() {
    let mut e = engine();
    e.set_goal("Goal".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step("t1".into(), "Step".into(), StepType::Implement, None, None)
        .unwrap();
    e.start_step("t1").unwrap();
    let check = e
        .record_check(
            None,
            CheckType::Test,
            "cargo test".into(),
            true,
            "42 passed".into(),
            true,
        )
        .unwrap()
        .value;
    assert!(check.passed);
    assert!(check.required);
}

#[test]
fn record_check_explicit_step() {
    let mut e = engine();
    e.set_goal("Goal".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step("t1".into(), "Step".into(), StepType::Implement, None, None)
        .unwrap();
    e.start_step("t1").unwrap();
    let check = e
        .record_check(
            Some("t1"),
            CheckType::Lint,
            "eslint".into(),
            false,
            "3 errors".into(),
            false,
        )
        .unwrap()
        .value;
    assert!(!check.passed);
    assert!(!check.required);
}

#[test]
fn verified_check_records_command_and_exit_code() {
    let mut e = engine();
    e.set_goal("Goal".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step("t1".into(), "Step".into(), StepType::Implement, None, None)
        .unwrap();
    e.start_step("t1").unwrap();
    let check = e
        .record_check_full(
            None,
            CheckType::Test,
            "cargo test".into(),
            true,
            "ran".into(),
            true,
            true,
            Some("cargo test".into()),
            Some(0),
            None,
            None,
        )
        .unwrap()
        .value;
    assert!(check.verified);
    assert_eq!(check.exit_code, Some(0));
    assert_eq!(check.command.as_deref(), Some("cargo test"));
}

#[test]
fn run_warns_about_self_reported_required_checks() {
    let mut e = engine();
    e.set_goal("Goal".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step("t1".into(), "Step".into(), StepType::Implement, None, None)
        .unwrap();
    e.start_step("t1").unwrap();
    // A self-reported (unverified) required pass — exactly the fabricated-green
    // case. run() must flag it rather than treat it as a real check.
    e.record_check(
        None,
        CheckType::Test,
        "tests".into(),
        true,
        "trust me".into(),
        true,
    )
    .unwrap();
    e.complete_step("t1", vec![]).unwrap();
    let report = serde_json::to_value(e.finalize(vec![], None).unwrap().value).unwrap();
    let warnings = report["warnings"].as_array().unwrap();
    assert!(
        warnings
            .iter()
            .any(|w| w.as_str().unwrap().contains("self-reported")),
        "expected a self-reported-check warning, got: {warnings:?}"
    );
}

// ── Run ───────────────────────────────────────────────────────────

#[test]
fn run_all_complete() {
    let mut e = engine();
    e.set_goal(
        "Goal".into(),
        vec!["it works".into()],
        vec![],
        String::new(),
    )
    .unwrap();
    e.add_step("t1".into(), "Step".into(), StepType::Implement, None, None)
        .unwrap();
    e.start_step("t1").unwrap();
    e.record_check_full(
        None,
        CheckType::Test,
        "test".into(),
        true,
        "ok".into(),
        true,
        true,
        Some("test".into()),
        Some(0),
        None,
        None,
    )
    .unwrap();
    e.complete_step("t1", vec![]).unwrap();
    let report =
        serde_json::to_value(e.finalize(vec![], Some("done".into())).unwrap().value).unwrap();
    assert_eq!(report["status"], "finished");
    assert_eq!(report["warnings"].as_array().unwrap().len(), 0);
    assert_eq!(e.goal.as_ref().unwrap().status, GoalStatus::Finished);
}

#[test]
fn run_warns_on_incomplete_steps() {
    let mut e = engine();
    e.set_goal("Goal".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step("t1".into(), "Step".into(), StepType::Implement, None, None)
        .unwrap();
    let report = serde_json::to_value(e.finalize(vec![], None).unwrap().value).unwrap();
    let warnings = report["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].as_str().unwrap().contains("not completed"));
}

#[test]
fn run_warns_on_failed_required_checks() {
    let mut e = engine();
    e.set_goal("Goal".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step("t1".into(), "Step".into(), StepType::Implement, None, None)
        .unwrap();
    e.start_step("t1").unwrap();
    e.record_check(
        None,
        CheckType::Test,
        "cargo test".into(),
        false,
        "failed".into(),
        true,
    )
    .unwrap();
    e.complete_step("t1", vec![]).unwrap();
    let report = serde_json::to_value(e.finalize(vec![], None).unwrap().value).unwrap();
    let warnings = report["warnings"].as_array().unwrap();
    assert!(
        warnings
            .iter()
            .any(|w| w.as_str().unwrap().contains("required checks failed"))
    );
}

// ── Status ─────────────────────────────────────────────────────────

#[test]
fn status_empty() {
    let e = engine();
    let s = e.status();
    assert!(s["goal"].is_null());
    assert_eq!(s["steps"]["total"], 0);
}

#[test]
fn status_with_decision_refs() {
    let mut e = engine();
    e.set_goal("Goal".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step(
        "t1".into(),
        "Step".into(),
        StepType::Implement,
        None,
        Some("branch-x".into()),
    )
    .unwrap();
    e.start_step("t1").unwrap();
    e.record_progress(
        None,
        ProgressType::Code,
        "x".into(),
        vec![],
        vec![],
        "".into(),
        Some(7),
    )
    .unwrap();
    let s = e.status();
    let refs = s["decision_refs"].as_array().unwrap();
    assert_eq!(refs.len(), 2);
    assert!(
        refs.iter()
            .any(|r| r["ref_type"] == "branch" && r["value"] == "branch-x")
    );
    assert!(
        refs.iter()
            .any(|r| r["ref_type"] == "step" && r["value"] == 7)
    );
}

#[test]
fn status_shows_project_id() {
    let e = engine();
    let s = e.status();
    assert_eq!(s["project_id"], "test-project-abc123");
}

// ── Export ──────────────────────────────────────────────────────────

#[test]
fn export_markdown() {
    let mut e = engine();
    e.set_goal(
        "Build it".into(),
        vec!["works".into()],
        vec![],
        String::new(),
    )
    .unwrap();
    e.add_step(
        "t1".into(),
        "Do thing".into(),
        StepType::Implement,
        None,
        None,
    )
    .unwrap();
    e.start_step("t1").unwrap();
    e.record_progress(
        None,
        ProgressType::Code,
        "wrote code".into(),
        vec![],
        vec![],
        "ok".into(),
        Some(3),
    )
    .unwrap();
    e.record_check(
        None,
        CheckType::Test,
        "cargo test".into(),
        true,
        "pass".into(),
        true,
    )
    .unwrap();
    let md = e.export("markdown");
    assert!(md.contains("Build it"));
    assert!(md.contains("Do thing"));
    assert!(md.contains("decision #3"));
    assert!(md.contains("[pass] cargo test"));
}

#[test]
fn export_json() {
    let mut e = engine();
    e.set_goal("Goal".into(), vec![], vec![], String::new())
        .unwrap();
    let json_str = e.export("json");
    let v: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert!(v["goal"].is_object());
}

// ── Reset ──────────────────────────────────────────────────────────

#[test]
fn reset_clears_everything() {
    let mut e = engine();
    e.set_goal("Goal".into(), vec![], vec![], String::new())
        .unwrap();
    e.add_step("t1".into(), "Step".into(), StepType::Implement, None, None)
        .unwrap();
    e.start_step("t1").unwrap();
    e.record_progress(
        None,
        ProgressType::Code,
        "x".into(),
        vec![],
        vec![],
        "".into(),
        None,
    )
    .unwrap();
    e.reset().unwrap();
    assert!(e.goal.is_none());
    assert!(e.steps.is_empty());
    let s = e.status();
    assert_eq!(s["steps"]["total"], 0);
}

// ── Full SDLC flow ─────────────────────────────────────────────────

fn planned_sdlc() -> RunEngine {
    let mut e = engine();

    e.set_goal(
        "Add JWT auth middleware".into(),
        vec!["JWT validation works".into(), "All tests pass".into()],
        vec!["No breaking changes".into()],
        "src/middleware/".into(),
    )
    .unwrap();

    e.add_step(
        "research".into(),
        "Research JWT libs".into(),
        StepType::Research,
        Some("small".into()),
        None,
    )
    .unwrap();
    e.add_step(
        "impl".into(),
        "Implement middleware".into(),
        StepType::Implement,
        Some("medium".into()),
        Some("jwt-approach".into()),
    )
    .unwrap();
    e.add_step(
        "test".into(),
        "Write tests".into(),
        StepType::Test,
        Some("small".into()),
        None,
    )
    .unwrap();
    e.add_step(
        "review".into(),
        "Self-review".into(),
        StepType::Review,
        Some("trivial".into()),
        None,
    )
    .unwrap();

    e
}

#[test]
fn full_sdlc_flow() {
    let mut e = planned_sdlc();

    e.start_step("research").unwrap();
    e.record_progress(
        None,
        ProgressType::Research,
        "evaluated jsonwebtoken vs jwt-simple".into(),
        vec![],
        vec![],
        "jsonwebtoken is better maintained".into(),
        Some(1),
    )
    .unwrap();
    e.complete_step("research", vec![]).unwrap();

    e.start_step("impl").unwrap();
    e.record_progress(
        None,
        ProgressType::Code,
        "added auth middleware".into(),
        vec!["src/middleware/auth.rs".into()],
        vec!["Edit".into()],
        "compiles".into(),
        Some(3),
    )
    .unwrap();
    e.record_progress(
        None,
        ProgressType::Code,
        "wired into router".into(),
        vec!["src/main.rs".into()],
        vec!["Edit".into()],
        "routes protected".into(),
        Some(4),
    )
    .unwrap();
    e.complete_step(
        "impl",
        vec![Artifact {
            artifact_type: ArtifactType::File,
            reference: "src/middleware/auth.rs".into(),
            description: "JWT validation middleware".into(),
        }],
    )
    .unwrap();

    e.start_step("test").unwrap();
    e.record_progress(
        None,
        ProgressType::Test,
        "wrote integration tests".into(),
        vec!["tests/auth.rs".into()],
        vec!["Edit".into()],
        "3 tests".into(),
        None,
    )
    .unwrap();
    e.record_check_full(
        None,
        CheckType::Test,
        "cargo test".into(),
        true,
        "45 tests pass".into(),
        true,
        true,
        Some("cargo test".into()),
        Some(0),
        None,
        None,
    )
    .unwrap();
    e.record_check_full(
        None,
        CheckType::Lint,
        "cargo clippy".into(),
        true,
        "no warnings".into(),
        true,
        true,
        Some("cargo clippy".into()),
        Some(0),
        None,
        None,
    )
    .unwrap();
    e.complete_step("test", vec![]).unwrap();

    e.start_step("review").unwrap();
    e.record_progress(
        None,
        ProgressType::Review,
        "reviewed all changes".into(),
        vec![],
        vec![],
        "looks good".into(),
        Some(8),
    )
    .unwrap();
    e.record_check(
        None,
        CheckType::Review,
        "self-review".into(),
        true,
        "approved".into(),
        false,
    )
    .unwrap();
    e.complete_step("review", vec![]).unwrap();

    let report = serde_json::to_value(
        e.finalize(
            vec![Artifact {
                artifact_type: ArtifactType::Commit,
                reference: "abc1234".into(),
                description: "feat: add JWT auth middleware".into(),
            }],
            Some("JWT auth middleware shipped".into()),
        )
        .unwrap()
        .value,
    )
    .unwrap();

    assert_eq!(report["status"], "finished");
    assert_eq!(report["steps"]["done"], 4);
    assert_eq!(report["steps"]["total"], 4);
    assert_eq!(report["warnings"].as_array().unwrap().len(), 0);

    let status = e.status();
    assert_eq!(status["goal"]["status"], "finished");
    assert_eq!(status["steps"]["done"], 4);

    let refs = status["decision_refs"].as_array().unwrap();
    assert!(
        refs.iter()
            .any(|r| r["ref_type"] == "branch" && r["value"] == "jwt-approach")
    );
    assert!(
        refs.iter()
            .any(|r| r["ref_type"] == "step" && r["value"] == 1)
    );
    assert!(
        refs.iter()
            .any(|r| r["ref_type"] == "step" && r["value"] == 3)
    );
    assert!(
        refs.iter()
            .any(|r| r["ref_type"] == "step" && r["value"] == 8)
    );
}
