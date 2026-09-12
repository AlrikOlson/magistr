//! All validation entry points must enforce the project's complete gate set.

#[path = "support/magistr_command.rs"]
mod magistr_command;

use std::path::Path;

fn write_project(dir: &Path) {
    let mut config: magistr_core::ProjectConfig = serde_json::from_value(serde_json::json!({
        "project": {"name":"validation-fixture"},
        "stack": {"language":"rust", "framework":"none", "package_manager":"cargo",
            "test_framework":"cargo-test", "linter":"clippy", "formatter":"rustfmt"},
        "quality_gate": {"check":"true", "test":"true", "lint":"true"},
        "format": {"command":"true", "extensions":[]},
        "testing": {"companion_pattern":"{name}.tests.{ext}", "test_dir":"alongside"},
        "reference_repos":[], "state_prefix":"validation-fixture",
        "custom_gates": [
            {"name":"required-extra", "command":"touch custom-ran; false", "required":true},
            {"name":"optional-extra", "command":"touch optional-ran; false", "required":false}
        ]
    }))
    .unwrap();
    let mut package = config.effective_archetypes().remove(0);
    package.id = "package".into();
    package.root = "package one".into();
    package.quality_gate.check = "touch archetype-ran; test -f package-marker".into();
    config.archetypes = vec![package];
    std::fs::create_dir_all(dir.join(".magistr")).unwrap();
    std::fs::create_dir(dir.join("package one")).unwrap();
    std::fs::write(dir.join("package one/package-marker"), "").unwrap();
    std::fs::write(
        dir.join(".magistr/project.json"),
        serde_json::to_vec(&config).unwrap(),
    )
    .unwrap();
}

#[test]
fn validation_modes_enforce_required_custom_and_archetype_gates() {
    for args in [
        vec!["validate"],
        vec!["validate", "--ci"],
        vec!["validate", "--json"],
        vec!["next", "--gate-only", "--format", "json"],
    ] {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("project");
        write_project(&project);
        let output = magistr_command::magistr_command()
            .args(&args)
            .arg(&project)
            .env("HOME", root.path())
            .env("XDG_CONFIG_HOME", root.path().join("config"))
            .env(
                "PATH",
                if cfg!(windows) {
                    std::env::var_os("PATH").unwrap_or_default()
                } else {
                    "/usr/bin:/bin".into()
                },
            )
            .current_dir(root.path())
            .output()
            .unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert_eq!(
            output.status.code(),
            Some(1),
            "{args:?}: {stdout}\n{stderr}"
        );
        let report = format!("{stdout}\n{stderr}");
        assert!(report.contains("required-extra"), "{args:?}: {report}");
        assert!(report.contains("check:package"), "{args:?}: {report}");
        assert!(project.join("custom-ran").exists(), "{args:?}");
        assert!(
            project.join("package one/archetype-ran").exists(),
            "{args:?}"
        );
        assert!(!project.join("optional-ran").exists(), "{args:?}");
        assert!(!root.path().join("custom-ran").exists(), "{args:?}");
    }
}

#[test]
fn cumulative_cost_limit_requires_headless_mode() {
    let output = magistr_command::magistr_command()
        .args(["next-loop", "--max-cost", "1.0"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("--headless")
    );
}
