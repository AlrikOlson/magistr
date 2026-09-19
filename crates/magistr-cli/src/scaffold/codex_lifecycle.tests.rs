use super::super::{apply::apply_approved, preview, remove_scaffold, repair};
use super::*;

fn codex_vars() -> HashMap<String, String> {
    let mut vars = super::tests::test_vars();
    vars.insert("selected_agents".into(), "codex".into());
    vars
}

#[test]
fn codex_setup_merges_shared_files_and_preview_matches_generation() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    std::fs::create_dir(dir.join(".codex")).unwrap();
    let existing =
        "# my setting\nmodel = 'chosen' # retain\n\n[mcp_servers.custom]\ncommand = 'mine'\n";
    std::fs::write(dir.join(".codex/config.toml"), existing).unwrap();
    std::fs::write(dir.join("AGENTS.md"), "# Team rules\nKeep this.\n").unwrap();
    let p = preview(
        dir,
        &codex_vars(),
        &HashMap::new(),
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();
    let config = p
        .files
        .iter()
        .find(|f| f.path == ".codex/config.toml")
        .expect("Codex configuration must be previewed")
        .new_content
        .clone();
    let agents = p
        .files
        .iter()
        .find(|f| f.path == "AGENTS.md")
        .unwrap()
        .new_content
        .clone();
    generate_with_result(
        dir,
        &codex_vars(),
        &HashMap::new(),
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.join(".codex/config.toml")).unwrap(),
        config
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("AGENTS.md")).unwrap(),
        agents
    );
    assert!(config.starts_with(existing), "{config}");
    assert!(agents.starts_with("# Team rules\nKeep this.\n"));
    let doc: toml::Value = toml::from_str(&config).unwrap();
    assert_eq!(
        doc["mcp_servers"]["magistr"]["command"].as_str(),
        Some("magistr")
    );
    assert_eq!(
        doc["mcp_servers"]["think-and-ship"]["enabled"].as_bool(),
        Some(false)
    );
    for name in [
        "next",
        "next-loop",
        "validate",
        "roadmap",
        "roadmap-refresh",
        "signals",
        "craft",
        "init",
        "workbench-routing",
    ] {
        assert!(
            dir.join(format!(".agents/skills/{name}/SKILL.md"))
                .is_file(),
            "{name}"
        );
    }
    let repeated = generate_with_result(
        dir,
        &codex_vars(),
        &HashMap::new(),
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();
    assert!(repeated.unchanged.contains(&".codex/config.toml".into()));
    assert!(repeated.unchanged.contains(&"AGENTS.md".into()));
}

#[test]
fn codex_malformed_config_rejects_all_scaffold_writes() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    std::fs::create_dir(dir.join(".codex")).unwrap();
    std::fs::write(dir.join(".codex/config.toml"), "[broken").unwrap();
    assert!(
        preview(
            dir,
            &codex_vars(),
            &HashMap::new(),
            &crate::scaffold::SetupSeeds::default()
        )
        .is_err()
    );
    assert!(
        generate_with_result(
            dir,
            &codex_vars(),
            &HashMap::new(),
            &crate::scaffold::SetupSeeds::default()
        )
        .is_err()
    );
    assert!(!dir.join(".magistr").exists());
    assert!(!dir.join(".claude").exists());
    assert_eq!(
        std::fs::read_to_string(dir.join(".codex/config.toml")).unwrap(),
        "[broken"
    );
}

#[test]
fn codex_selective_apply_records_ownership_and_preserves_customizations() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let p = preview(
        dir,
        &codex_vars(),
        &HashMap::new(),
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();
    let selected: Vec<_> = p
        .files
        .iter()
        .filter(|f| {
            f.path == ".codex/config.toml"
                || f.path == "AGENTS.md"
                || f.path.starts_with(".agents/")
        })
        .collect();
    assert!(!selected.is_empty(), "Codex must produce reviewable files");
    apply_approved(dir, &selected).unwrap();
    let config_path = dir.join(".codex/config.toml");
    let config = std::fs::read_to_string(&config_path)
        .unwrap()
        .replace("command = \"magistr\"", "command = \"custom-wrapper\"");
    std::fs::write(&config_path, &config).unwrap();
    let skill = dir.join(".agents/skills/next/SKILL.md");
    std::fs::write(&skill, "my workflow").unwrap();
    generate_with_result(
        dir,
        &codex_vars(),
        &HashMap::new(),
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();
    assert_eq!(std::fs::read_to_string(&skill).unwrap(), "my workflow");
    assert!(
        std::fs::read_to_string(&config_path)
            .unwrap()
            .contains("custom-wrapper")
    );
    let _ = repair::repair(dir);
    assert_eq!(std::fs::read_to_string(&skill).unwrap(), "my workflow");
    remove_scaffold(dir).unwrap();
    assert_eq!(std::fs::read_to_string(&skill).unwrap(), "my workflow");
    assert!(
        std::fs::read_to_string(&config_path)
            .unwrap()
            .contains("custom-wrapper")
    );
    assert!(!dir.join(".agents/skills/validate/SKILL.md").exists());
}
