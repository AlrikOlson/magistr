use super::*;

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
    let ship = ShipGuardState {
        objective_set: true,
        tasks_planned: 2,
        checks_recorded: 0,
    };
    let ctx = GuardContext::new("/project", "/project/SPEC.md", ship);
    assert_eq!(ctx.project_dir, "/project");
    assert_eq!(ctx.spec_path, "/project/SPEC.md");
    assert_eq!(ctx.ship, ship);
    assert!(ctx.ship.has_plan());
}

// ── WorkflowGuard trait defaults ────────────────────────────────────

#[test]
fn default_trait_methods_return_ok() {
    struct TestGuard;
    impl WorkflowGuard for TestGuard {
        fn name(&self) -> &str {
            "test"
        }
    }

    let guard = TestGuard;
    let ctx = GuardContext::new("/p", "/p/SPEC.md", ShipGuardState::default());
    assert!(guard.pre_session(&ctx).is_ok());
    assert!(guard.post_session(&ctx).is_ok());
    assert!(guard.pre_commit(&ctx).is_ok());
    assert!(guard.post_commit(&ctx).is_ok());
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
