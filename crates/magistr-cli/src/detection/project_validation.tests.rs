use super::*;
use serde_json::json;

#[test]
fn a_consolidated_clippy_result_satisfies_only_the_exact_check_contract() {
    let check = magistr_core::gates::GateSpec {
        name: "check".into(),
        command: "cargo check --all-targets".into(),
        required: true,
    };
    let run = GateRun {
        name: "lint".into(),
        command: "cargo clippy --all-targets -- -D warnings".into(),
        required: true,
        verdict: crate::utils::gate::GateVerdict::Passed,
        exit_code: 0,
        output: String::new(),
        rows: vec![],
        duration: Duration::ZERO,
        queue_duration: Duration::ZERO,
    };
    let mut result = ProjectValidation {
        results: vec![run],
        checks: vec![check],
    };
    let policy = GovernancePolicy {
        required_gates: vec!["check".into()],
        ..Default::default()
    };
    assert!(result.policy_violations(&policy).is_empty());
    result.checks[0].command.push_str(" --features another");
    assert_eq!(result.policy_violations(&policy), vec!["check"]);
}

fn config() -> crate::types::ProjectConfig {
    serde_json::from_value(json!({
        "project": {"name": "gate-fixture"},
        "stack": {"language":"rust", "framework":"none", "package_manager":"cargo",
            "test_framework":"cargo-test", "linter":"clippy", "formatter":"rustfmt"},
        "quality_gate": {"check":"printf run >> root-runs; test -f root-marker",
            "test":"true", "lint":"true"},
        "format": {"command":"true", "extensions":[]},
        "testing": {"companion_pattern":"{name}.tests.{ext}", "test_dir":"alongside"},
        "reference_repos":[], "state_prefix":"gate-fixture",
        "custom_gates": [
            {"name":"required-extra", "command":"printf fail >> custom-runs; false", "required":true},
            {"name":"optional-extra", "command":"touch optional-ran; false", "required":false}
        ]
    }))
    .unwrap()
}

#[tokio::test]
async fn project_gates_include_required_custom_checks_and_resolve_archetype_directories() {
    let root = tempfile::tempdir().unwrap();
    let mut config = config();
    let mut shared = config.effective_archetypes().remove(0);
    shared.id = "shared".into();
    shared.root = "package one".into();
    let mut duplicate = shared.clone();
    duplicate.id = "duplicate".into();
    let mut package = shared.clone();
    package.id = "package".into();
    package.quality_gate.check = "test -f package-marker".into();
    config.archetypes = vec![shared, duplicate, package];
    std::fs::create_dir(root.path().join(".magistr")).unwrap();
    std::fs::create_dir(root.path().join("package one")).unwrap();
    std::fs::write(root.path().join("root-marker"), "").unwrap();
    std::fs::write(root.path().join("package one/package-marker"), "").unwrap();
    std::fs::write(
        root.path().join(".magistr/project.json"),
        serde_json::to_vec(&config).unwrap(),
    )
    .unwrap();

    let result = ProjectValidation::run(root.path()).await.unwrap();
    assert!(!result.all_passed());
    assert_eq!(result.failed_count(), 1);
    assert_eq!(result.results.len(), 5);
    assert_eq!(
        std::fs::read_to_string(root.path().join("root-runs")).unwrap(),
        "run"
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("custom-runs")).unwrap(),
        "fail"
    );
    assert!(!root.path().join("optional-ran").exists());
    assert!(
        result
            .results
            .iter()
            .find(|g| g.name == "check:package")
            .unwrap()
            .passed()
    );

    let output = serde_json::to_value(ValidationOutput::from(&result)).unwrap();
    assert_eq!(output["all_passed"], false);
    assert!(
        output["gates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["name"] == "required-extra" && g["passed"] == false)
    );
    let policy = GovernancePolicy {
        required_gates: vec!["check".into(), "required-extra".into()],
        ..GovernancePolicy::default()
    };
    assert_eq!(result.policy_violations(&policy), vec!["required-extra"]);
}
