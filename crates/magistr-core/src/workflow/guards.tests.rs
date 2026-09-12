use std::process::Command;

use magistr_engine::ShipGuardState;

use super::*;

/// A project with no ship state at all — no objective, no tasks, no checks.
fn temp_project() -> (tempfile::TempDir, GuardContext) {
    temp_project_with(ShipGuardState::default())
}

fn temp_project_with(ship: ShipGuardState) -> (tempfile::TempDir, GuardContext) {
    let dir = tempfile::TempDir::new().unwrap();
    let spec_path = dir.path().join("ROADMAP.md");
    let ctx = GuardContext::new(
        dir.path().to_string_lossy().to_string(),
        spec_path.to_string_lossy().to_string(),
        ship,
    );
    (dir, ctx)
}

/// Objective set, one task planned, no gate recorded yet.
fn planned() -> ShipGuardState {
    ShipGuardState {
        objective_set: true,
        tasks_planned: 1,
        checks_recorded: 0,
    }
}

// ── GuardResult tests ───────────────────────────────────────────────

#[test]
fn guard_result_ok_is_ok() {
    let r = GuardResult::ok("test");
    assert!(r.is_ok());
    assert_eq!(r.guard_name, "test");
    assert_eq!(r.action, GuardAction::Continue);
}

#[test]
fn guard_result_steer_is_not_ok() {
    let r = GuardResult::steer("test", "do this");
    assert!(!r.is_ok());
    assert_eq!(r.action, GuardAction::SteerPrompt("do this".into()));
}

#[test]
fn guard_result_warn_is_not_ok() {
    let r = GuardResult::warn("test", "heads up");
    assert!(!r.is_ok());
    assert_eq!(r.action, GuardAction::Warn("heads up".into()));
}

// ── GuardContext tests ──────────────────────────────────────────────

#[test]
fn guard_context_construction() {
    let ctx = GuardContext::new("/project", "/project/SPEC.md", planned());
    assert_eq!(ctx.project_dir, "/project");
    assert_eq!(ctx.spec_path, "/project/SPEC.md");
    assert_eq!(ctx.ship, planned());
}

// ── PlanGuard tests ─────────────────────────────────────────────────

#[test]
fn plan_guard_name() {
    assert_eq!(PlanGuard.name(), "plan");
}

#[test]
fn plan_guard_steers_when_no_ship_plan() {
    let (_dir, ctx) = temp_project();
    let result = PlanGuard.pre_session(&ctx);
    assert!(!result.is_ok());
    match result.action {
        GuardAction::SteerPrompt(msg) => {
            assert!(msg.contains("ship_set_objective"), "{msg}");
            assert!(msg.contains("ship_plan"), "{msg}");
            assert!(!msg.contains("current-plan.md"), "{msg}");
        }
        other => panic!("expected steer, got {other:?}"),
    }
}

#[test]
fn plan_guard_steers_when_objective_but_no_task() {
    let (_dir, ctx) = temp_project_with(ShipGuardState {
        objective_set: true,
        tasks_planned: 0,
        checks_recorded: 0,
    });
    assert!(!PlanGuard.pre_session(&ctx).is_ok());
}

#[test]
fn plan_guard_continues_when_ship_plan_recorded() {
    let (_dir, ctx) = temp_project_with(planned());
    assert!(PlanGuard.pre_session(&ctx).is_ok());
}

#[test]
fn plan_guard_pre_commit_needs_a_recorded_check() {
    let (_dir, ctx) = temp_project_with(planned());
    let result = PlanGuard.pre_commit(&ctx);
    assert!(!result.is_ok());
    assert!(matches!(result.action, GuardAction::SteerPrompt(ref m) if m.contains("ship_check")));

    let (_dir, ctx) = temp_project_with(ShipGuardState {
        checks_recorded: 1,
        ..planned()
    });
    assert!(PlanGuard.pre_commit(&ctx).is_ok());
}

#[test]
fn plan_guard_default_hooks_are_ok() {
    let (_dir, ctx) = temp_project();
    assert!(PlanGuard.post_session(&ctx).is_ok());
    assert!(PlanGuard.post_commit(&ctx).is_ok());
}

// ── SpecGuard tests ─────────────────────────────────────────────────

#[test]
fn spec_guard_name() {
    assert_eq!(SpecGuard.name(), "spec");
}

#[test]
fn spec_guard_steers_when_spec_unmodified() {
    let dir = tempfile::TempDir::new().unwrap();
    Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let spec_path = dir.path().join("ROADMAP.md");
    std::fs::write(&spec_path, "- [x] Done\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let ctx = GuardContext::new(
        dir.path().to_string_lossy().to_string(),
        spec_path.to_string_lossy().to_string(),
        planned(),
    );

    let result = SpecGuard.post_session(&ctx);
    assert!(!result.is_ok());
    assert!(matches!(result.action, GuardAction::SteerPrompt(_)));
}

#[test]
fn spec_guard_continues_when_spec_modified() {
    let dir = tempfile::TempDir::new().unwrap();
    Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let spec_path = dir.path().join("ROADMAP.md");
    std::fs::write(&spec_path, "- [x] Done\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    // Now modify the spec
    std::fs::write(&spec_path, "- [x] Done\n- [x] New item\n").unwrap();

    let ctx = GuardContext::new(
        dir.path().to_string_lossy().to_string(),
        spec_path.to_string_lossy().to_string(),
        planned(),
    );

    let result = SpecGuard.post_session(&ctx);
    assert!(result.is_ok());
}

#[test]
fn spec_guard_continues_when_spec_staged() {
    let dir = tempfile::TempDir::new().unwrap();
    Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let spec_path = dir.path().join("ROADMAP.md");
    std::fs::write(&spec_path, "- [x] Done\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    // Modify and stage
    std::fs::write(&spec_path, "- [x] Done\n- [x] Staged\n").unwrap();
    Command::new("git")
        .args(["add", "ROADMAP.md"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let ctx = GuardContext::new(
        dir.path().to_string_lossy().to_string(),
        spec_path.to_string_lossy().to_string(),
        planned(),
    );

    let result = SpecGuard.post_session(&ctx);
    assert!(result.is_ok());
}

#[test]
fn spec_guard_default_hooks_are_ok() {
    let (_dir, ctx) = temp_project();
    assert!(SpecGuard.pre_session(&ctx).is_ok());
    assert!(SpecGuard.pre_commit(&ctx).is_ok());
    assert!(SpecGuard.post_commit(&ctx).is_ok());
}

// ── CleanTreeGuard tests ────────────────────────────────────────────

#[test]
fn clean_tree_guard_name() {
    assert_eq!(CleanTreeGuard.name(), "clean_tree");
}

#[test]
fn clean_tree_guard_ok_when_clean() {
    let dir = tempfile::TempDir::new().unwrap();
    Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    std::fs::write(dir.path().join("README.md"), "# Hello\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let ctx = GuardContext::new(
        dir.path().to_string_lossy().to_string(),
        dir.path().join("ROADMAP.md").to_string_lossy().to_string(),
        planned(),
    );

    let result = CleanTreeGuard.post_session(&ctx);
    assert!(result.is_ok());
}

#[test]
fn clean_tree_guard_warns_when_dirty() {
    let dir = tempfile::TempDir::new().unwrap();
    Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    std::fs::write(dir.path().join("dirty.txt"), "changes\n").unwrap();

    let ctx = GuardContext::new(
        dir.path().to_string_lossy().to_string(),
        dir.path().join("ROADMAP.md").to_string_lossy().to_string(),
        planned(),
    );

    let result = CleanTreeGuard.post_session(&ctx);
    assert!(!result.is_ok());
    assert!(matches!(result.action, GuardAction::Warn(_)));
}

#[test]
fn clean_tree_guard_default_hooks_are_ok() {
    let (_dir, ctx) = temp_project();
    assert!(CleanTreeGuard.pre_session(&ctx).is_ok());
    assert!(CleanTreeGuard.pre_commit(&ctx).is_ok());
    assert!(CleanTreeGuard.post_commit(&ctx).is_ok());
}

// ── GuardsConfig tests ──────────────────────────────────────────────

#[test]
fn guards_config_default_all_enabled() {
    let config = GuardsConfig::default();
    assert!(config.plan);
    assert!(config.spec);
    assert!(config.clean_tree);
    assert!(config.quality_gate);
}

#[test]
fn guards_config_deserialize_partial() {
    let json = r#"{"plan": false}"#;
    let config: GuardsConfig = serde_json::from_str(json).unwrap();
    assert!(!config.plan);
    assert!(config.spec);
    assert!(config.clean_tree);
}

#[test]
fn guards_config_deserialize_empty() {
    let json = "{}";
    let config: GuardsConfig = serde_json::from_str(json).unwrap();
    assert!(config.plan);
    assert!(config.spec);
    assert!(config.clean_tree);
}

#[test]
fn guards_config_roundtrip() {
    let config = GuardsConfig {
        plan: true,
        spec: false,
        clean_tree: true,
        quality_gate: true,
    };
    let json = serde_json::to_string(&config).unwrap();
    let parsed: GuardsConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(config, parsed);
}

// ── GuardRunner tests ───────────────────────────────────────────────

#[test]
fn runner_from_config_all_enabled() {
    let config = GuardsConfig::default();
    let runner = GuardRunner::from_config(&config);
    assert_eq!(runner.len(), 4);
    assert!(!runner.is_empty());
}

#[test]
fn runner_from_config_none_enabled() {
    let config = GuardsConfig {
        plan: false,
        spec: false,
        clean_tree: false,
        quality_gate: false,
    };
    let runner = GuardRunner::from_config(&config);
    assert_eq!(runner.len(), 0);
    assert!(runner.is_empty());
}

#[test]
fn runner_from_config_partial() {
    let config = GuardsConfig {
        plan: true,
        spec: false,
        clean_tree: true,
        quality_gate: false,
    };
    let runner = GuardRunner::from_config(&config);
    assert_eq!(runner.len(), 2);
}

#[test]
fn runner_pre_session_collects_non_ok() {
    let (_dir, ctx) = temp_project();
    let runner = GuardRunner::from_config(&GuardsConfig::default());
    let results = runner.run_pre_session(&ctx);
    assert!(
        results.iter().any(|r| r.guard_name == "plan"),
        "PlanGuard should fire when no ship plan is recorded"
    );
}

#[test]
fn runner_pre_session_empty_when_all_ok() {
    let (_dir, ctx) = temp_project_with(planned());

    let runner = GuardRunner::from_config(&GuardsConfig::default());
    let results = runner.run_pre_session(&ctx);
    assert!(results.is_empty(), "All guards should be ok");
}

#[test]
fn runner_disabled_guards_are_skipped() {
    let (_dir, ctx) = temp_project();
    let config = GuardsConfig {
        plan: false,
        spec: false,
        clean_tree: false,
        quality_gate: false,
    };
    let runner = GuardRunner::from_config(&config);
    let results = runner.run_pre_session(&ctx);
    assert!(results.is_empty());
}

#[test]
fn runner_guards_compose_in_order() {
    struct FirstGuard;
    impl WorkflowGuard for FirstGuard {
        fn name(&self) -> &str {
            "first"
        }
        fn pre_session(&self, _ctx: &GuardContext) -> GuardResult {
            GuardResult::warn("first", "first warning")
        }
    }

    struct SecondGuard;
    impl WorkflowGuard for SecondGuard {
        fn name(&self) -> &str {
            "second"
        }
        fn pre_session(&self, _ctx: &GuardContext) -> GuardResult {
            GuardResult::warn("second", "second warning")
        }
    }

    let runner = GuardRunner::new(vec![Box::new(FirstGuard), Box::new(SecondGuard)]);
    let (_dir, ctx) = temp_project();
    let results = runner.run_pre_session(&ctx);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].guard_name, "first");
    assert_eq!(results[1].guard_name, "second");
}

#[test]
fn runner_post_session_works() {
    let (_dir, ctx) = temp_project();
    let runner = GuardRunner::new(Vec::new());
    let results = runner.run_post_session(&ctx);
    assert!(results.is_empty());
}

#[test]
fn runner_pre_commit_works() {
    let (_dir, ctx) = temp_project();
    let runner = GuardRunner::new(Vec::new());
    let results = runner.run_pre_commit(&ctx);
    assert!(results.is_empty());
}

#[test]
fn runner_post_commit_works() {
    let (_dir, ctx) = temp_project();
    let runner = GuardRunner::new(Vec::new());
    let results = runner.run_post_commit(&ctx);
    assert!(results.is_empty());
}
