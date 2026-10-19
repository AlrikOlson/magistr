use super::*;
use serde_json::json;

fn rig() -> (tempfile::TempDir, ResolveOptions) {
    let temp = tempfile::tempdir().unwrap();
    let mut options = ResolveOptions::new(temp.path());
    options.env.insert(
        "MAGISTR_WORKSPACE_ORG_POLICY".into(),
        temp.path().join("org.toml").display().to_string(),
    );
    options.env.insert(
        "MAGISTR_WORKSPACE_USER_CONFIG".into(),
        temp.path().join("user.toml").display().to_string(),
    );
    (temp, options)
}

#[test]
fn policy_ceiling_refuses_workspace_and_session_loosenings() {
    let (temp, mut options) = rig();
    std::fs::write(
        temp.path().join("org.toml"),
        "[security]\nredaction=true\nallow_sideload=false\n[telemetry]\nconsent='crash'\n",
    )
    .unwrap();
    std::fs::create_dir_all(temp.path().join(".magistr")).unwrap();
    std::fs::write(
        temp.path().join(".magistr/workspace.toml"),
        "[security]\nredaction=false\n[telemetry]\nconsent='off'\n",
    )
    .unwrap();
    options
        .session_flags
        .insert("security.allow_sideload".into(), json!(true));
    let config = ResolvedConfig::resolve(&options).unwrap();
    assert_eq!(config.effective["security.redaction"], json!(true));
    assert_eq!(config.effective["security.allow_sideload"], json!(false));
    assert_eq!(config.effective["telemetry.consent"], json!("off"));
    assert_eq!(config.violations.len(), 2);
    assert_eq!(config.violations[0].pinned_by, ConfigLayer::OrgPolicy);
    assert_eq!(config.violations[1].attempted_by, ConfigLayer::Session);
}

#[test]
fn empty_org_allowlist_cannot_gain_workspace_commands() {
    let (temp, options) = rig();
    std::fs::write(temp.path().join("org.toml"), "[commands]\n").unwrap();
    std::fs::create_dir_all(temp.path().join(".magistr")).unwrap();
    std::fs::write(
        temp.path().join(".magistr/workspace.toml"),
        "[commands]\ntest='cargo test'\n",
    )
    .unwrap();
    let config = ResolvedConfig::resolve(&options).unwrap();
    assert!(config.commands.is_empty());
    assert_eq!(config.violations.len(), 1);
    assert_eq!(config.violations[0].key, "commands.test");
}

#[test]
fn defaults_do_not_pin_and_paths_union_without_removal() {
    let (temp, mut options) = rig();
    std::fs::write(
        temp.path().join("user.toml"),
        "[security]\nprotected_paths=['secrets/**']\n[edit]\nforce_confirm=true\n",
    )
    .unwrap();
    std::fs::create_dir_all(temp.path().join(".magistr")).unwrap();
    std::fs::write(temp.path().join(".magistr/workspace.toml"), "[security]\nredaction=false\nprotected_paths=['deploy/**','secrets/**']\n[budgets]\ndefault=2000\n").unwrap();
    options
        .session_flags
        .insert("edit.force_confirm".into(), json!(false));
    let config = ResolvedConfig::resolve(&options).unwrap();
    assert_eq!(config.effective["security.redaction"], json!(false));
    assert_eq!(
        config.effective["security.protected_paths"],
        json!(["secrets/**", "deploy/**"])
    );
    assert_eq!(config.effective["budgets.default"], json!(2000));
    assert_eq!(config.violations[0].pinned_by, ConfigLayer::User);
}

#[test]
fn future_version_refuses_but_bad_layers_and_types_warn() {
    let (temp, options) = rig();
    std::fs::write(temp.path().join("org.toml"), "broken [ =").unwrap();
    std::fs::create_dir_all(temp.path().join(".magistr")).unwrap();
    std::fs::write(
        temp.path().join(".magistr/workspace.toml"),
        "unknown=1\n[security]\nredaction='yes'\n",
    )
    .unwrap();
    let config = ResolvedConfig::resolve(&options).unwrap();
    assert_eq!(config.warnings.len(), 3);
    assert!(config.layers[1].error.is_some());
    std::fs::write(
        temp.path().join(".magistr/workspace.toml"),
        "config_version=99",
    )
    .unwrap();
    assert!(ResolvedConfig::resolve(&options).is_err());
}

#[test]
fn commands_merge_nearest_wins_and_doctor_matches_provenance() {
    let (temp, options) = rig();
    std::fs::write(
        temp.path().join("user.toml"),
        "[commands]\nz='old'\na='cargo test'\n[security]\nallow_sideload=false\n",
    )
    .unwrap();
    std::fs::create_dir_all(temp.path().join(".magistr")).unwrap();
    std::fs::write(
        temp.path().join(".magistr/workspace.toml"),
        "[commands]\nz='new'\n[budgets]\ndefault=2500\n",
    )
    .unwrap();
    let config = ResolvedConfig::resolve(&options).unwrap();
    assert_eq!(config.commands["z"], "new");
    assert_eq!(
        config
            .trace
            .iter()
            .skip(10)
            .map(|key| key.key.as_str())
            .collect::<Vec<_>>(),
        ["commands.a", "commands.z"]
    );
    let report = config.doctor(temp.path());
    assert!(report.contains("security.allow_sideload = false  (user, pinned by user)"));
    assert!(report.contains("budgets.default = 2500  (workspace)"));
    assert!(report.contains("security.redaction = true  (defaults)"));
    assert!(!report.ends_with('\n'));
    assert_eq!(config, ResolvedConfig::resolve(&options).unwrap());
}

#[test]
fn integral_float_future_version_fails_closed_like_reference() {
    let (temp, options) = rig();
    std::fs::create_dir_all(temp.path().join(".magistr")).unwrap();
    std::fs::write(
        temp.path().join(".magistr/workspace.toml"),
        "config_version=2.0",
    )
    .unwrap();
    assert!(ResolvedConfig::resolve(&options).is_err());
}
