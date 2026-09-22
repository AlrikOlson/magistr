use super::*;
use crate::scaffold::{
    FileAction, apply::apply_approved, generate_with_result, pipeline::ScaffoldPipeline,
};

/// Plan and apply the Codex repair, as the startup repair does for it.
fn repair_codex(dir: &Path) -> Result<Vec<String>> {
    let changes = CodexSetup::repair_plan(dir)?;
    let owned = changes
        .iter()
        .map(|c| c.path.as_str())
        .filter(|p| CodexSetup::owned_path(p));
    let mut service =
        crate::scaffold::AdminService::open(dir, crate::scaffold::AdminOperation::Repair, owned);
    for change in &changes {
        service.execute(change);
    }
    let receipt = service.finish();
    match receipt.partial_error() {
        Some(error) => Err(error),
        None => Ok(receipt.applied_paths()),
    }
}
use std::collections::BTreeMap;

fn vars() -> HashMap<String, String> {
    ScaffoldPipeline::headless(crate::types::Stack::default())
        .with_agents(&["codex".to_owned()])
        .template_vars(tempfile::tempdir().unwrap().path())
}

fn setup(dir: &Path) {
    generate_with_result(
        dir,
        &vars(),
        &HashMap::new(),
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();
}

fn write(dir: &Path, path: &str, content: &str) {
    let path = dir.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn read(dir: &Path, path: &str) -> String {
    std::fs::read_to_string(dir.join(path)).unwrap()
}

#[derive(Debug, PartialEq, Eq)]
enum Entry {
    Directory,
    File(Vec<u8>),
    Symlink(std::path::PathBuf),
}

fn snapshot(dir: &Path) -> BTreeMap<std::path::PathBuf, Entry> {
    fn visit(root: &Path, dir: &Path, entries: &mut BTreeMap<std::path::PathBuf, Entry>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let metadata = std::fs::symlink_metadata(&path).unwrap();
            let value = if metadata.is_symlink() {
                Entry::Symlink(std::fs::read_link(&path).unwrap())
            } else if metadata.is_dir() {
                visit(root, &path, entries);
                Entry::Directory
            } else {
                Entry::File(std::fs::read(&path).unwrap())
            };
            entries.insert(path.strip_prefix(root).unwrap().to_owned(), value);
        }
    }
    let mut entries = BTreeMap::new();
    visit(dir, dir, &mut entries);
    entries
}

#[test]
fn recovered_codex_selection_keeps_all_headless_default_emitters() {
    let tmp = tempfile::tempdir().unwrap();
    setup(tmp.path());
    let mut defaults = vars();
    defaults.remove("selected_agents");
    let preview = crate::scaffold::preview(
        tmp.path(),
        &defaults,
        &HashMap::new(),
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();
    for path in [
        CONFIG,
        ".cursor/rules/workflow.mdc",
        ".github/copilot-instructions.md",
        ".clinerules",
        ".windsurfrules",
        ".aider.conf.yml",
    ] {
        assert!(preview.files.iter().any(|file| file.path == path), "{path}");
    }
}

#[test]
fn target_removal_preserves_recorded_skills_outside_its_candidate_set() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);
    let mut manifest = ScaffoldManifest::load(dir);
    for path in [
        ".agents/skills/team/SKILL.md",
        ".agents/skills/next/user-notes.md",
    ] {
        write(dir, path, "owned by a different scaffold component\n");
        manifest.record(path, "owned by a different scaffold component\n");
    }
    manifest.save(dir).unwrap();
    let changed = CodexSetup::remove_target(dir).unwrap();
    for path in [
        ".agents/skills/team/SKILL.md",
        ".agents/skills/next/user-notes.md",
    ] {
        assert_eq!(read(dir, path), "owned by a different scaffold component\n");
        assert!(!changed.iter().any(|value| value == path));
        assert!(ScaffoldManifest::load(dir).files.contains_key(path));
    }
}

#[test]
fn removal_without_codex_ownership_preserves_absent_and_broken_manifests() {
    for manifest in [None, Some("{broken"), Some(r#"{"files":{}}"#)] {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write(dir, CONFIG, "model = 'user-choice'\n");
        write(dir, AGENTS, "User instructions\n");
        if let Some(content) = manifest {
            write(dir, ".magistr/scaffold-manifest.json", content);
        }
        let before = snapshot(dir);
        if let Ok(changed) = CodexSetup::remove_target(dir) {
            assert!(changed.is_empty());
        }
        assert_eq!(snapshot(dir), before);
    }
}

#[test]
#[cfg(unix)]
fn removal_rejects_symlinked_ownership_evidence_without_external_writes() {
    use std::os::unix::fs::symlink;

    for path in [".magistr", ".magistr/scaffold-manifest.json"] {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        setup(dir);
        let moved = outside.path().join("ownership");
        std::fs::rename(dir.join(path), &moved).unwrap();
        symlink(&moved, dir.join(path)).unwrap();
        let before = snapshot(dir);
        let external_before = snapshot(outside.path());
        assert!(CodexSetup::remove_target(dir).is_err(), "{path}");
        assert_eq!(snapshot(dir), before);
        assert_eq!(snapshot(outside.path()), external_before);
    }
}

#[test]
fn removal_receipt_includes_deleted_skills_and_launcher_but_not_customized_files() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);
    let protected = ".agents/skills/next/SKILL.md";
    write(dir, protected, "User's workflow\n");
    let manifest = ScaffoldManifest::load(dir);
    let changed = CodexSetup::remove_target(dir).unwrap();
    for path in manifest
        .files
        .keys()
        .filter(|path| CodexSetup::owned_path(path))
    {
        if path == protected {
            assert!(!changed.contains(path));
            assert_eq!(read(dir, path), "User's workflow\n");
        } else {
            assert!(changed.contains(path), "missing receipt for {path}");
            assert!(!dir.join(path).exists(), "{path}");
        }
    }
    assert!(changed.iter().any(|path| path == CONFIG));
    assert!(changed.iter().any(|path| path == AGENTS));
}

#[test]
fn malformed_instruction_markers_reject_every_codex_mutation_before_writing() {
    for malformed in [
        format!("{START}\nUnterminated\n"),
        format!("{END}\n{START}\n"),
        format!("{START}\none\n{END}\n{START}\ntwo\n{END}\n"),
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        setup(dir);
        write(dir, AGENTS, &malformed);
        let before = snapshot(dir);
        assert!(
            crate::scaffold::preview(
                dir,
                &vars(),
                &HashMap::new(),
                &crate::scaffold::SetupSeeds::default()
            )
            .is_err()
        );
        assert_eq!(snapshot(dir), before);
        assert!(
            generate_with_result(
                dir,
                &vars(),
                &HashMap::new(),
                &crate::scaffold::SetupSeeds::default()
            )
            .is_err()
        );
        assert_eq!(snapshot(dir), before);
        assert!(repair_codex(dir).is_err());
        assert_eq!(snapshot(dir), before);
        assert!(CodexSetup::remove_target(dir).is_err());
        assert_eq!(snapshot(dir), before);
    }
}

#[test]
#[cfg(unix)]
fn setup_rejects_symlinked_codex_and_shared_skill_ancestors_before_writing() {
    use std::os::unix::fs::symlink;

    for path in [".codex", ".agents", ".agents/skills", ".agents/skills/next"] {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let link = dir.join(path);
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        write(outside.path(), "sentinel.txt", "Do not change\n");
        symlink(outside.path(), &link).unwrap();
        let before = snapshot(dir);
        let external_before = snapshot(outside.path());
        assert!(
            crate::scaffold::preview(
                dir,
                &vars(),
                &HashMap::new(),
                &crate::scaffold::SetupSeeds::default()
            )
            .is_err(),
            "{path}"
        );
        assert!(
            generate_with_result(
                dir,
                &vars(),
                &HashMap::new(),
                &crate::scaffold::SetupSeeds::default()
            )
            .is_err(),
            "{path}"
        );
        assert_eq!(snapshot(dir), before);
        assert_eq!(snapshot(outside.path()), external_before);
    }
}

#[test]
fn native_only_denials_and_disabled_standalone_server_restore_original_user_values() {
    for original_denials in ["['custom_guard']", "['custom_guard', 'roadmap_export']"] {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let project = "{\"roadmap_backend\":\"think-and-ship\",\"roadmap_exports\":false}\n";
        write(dir, ".magistr/project.json", project);
        let original = format!(
            "# User settings\nmodel = 'chosen'\n\n[mcp_servers.magistr]\ncommand = 'custom-magistr'\nargs = ['custom-root']\ndisabled_tools = {original_denials} # preserve original denials\n\n[mcp_servers.think-and-ship]\ncommand = 'custom-workflow'\nargs = ['serve', 'custom-root']\nenabled = true # restore this choice on removal\n"
        );
        write(dir, CONFIG, &original);
        setup(dir);
        setup(dir);
        repair_codex(dir).unwrap();
        let content = read(dir, CONFIG);
        let config: toml::Value = toml::from_str(&content).unwrap();
        assert_eq!(
            config["mcp_servers"]["magistr"]["command"].as_str(),
            Some("custom-magistr")
        );
        assert_eq!(
            config["mcp_servers"]["think-and-ship"]["enabled"].as_bool(),
            Some(false)
        );
        let denied = config["mcp_servers"]["magistr"]["disabled_tools"]
            .as_array()
            .unwrap();
        assert_eq!(
            denied
                .iter()
                .filter(|value| value.as_str() == Some("roadmap_export"))
                .count(),
            1
        );
        assert!(
            denied
                .iter()
                .any(|value| value.as_str() == Some("custom_guard"))
        );
        assert_eq!(read(dir, ".magistr/project.json"), project);
        assert!(!dir.join("ROADMAP.md").exists());
        assert!(!dir.join("ROADMAP.json").exists());
        CodexSetup::remove_target(dir).unwrap();
        assert_eq!(read(dir, CONFIG), original);
        assert_eq!(read(dir, ".magistr/project.json"), project);
    }
}

#[test]
fn customized_managed_instruction_block_survives_reinit_repair_and_removal() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write(dir, AGENTS, "# Team preface\n");
    setup(dir);
    let customized = format!(
        "{}\n# Team footer\n",
        read(dir, AGENTS).replace("## Magistr in Codex", "## Team's customized workflow")
    );
    write(dir, AGENTS, &customized);
    setup(dir);
    assert_eq!(read(dir, AGENTS), customized);
    repair_codex(dir).unwrap();
    assert_eq!(read(dir, AGENTS), customized);
    CodexSetup::remove_target(dir).unwrap();
    assert_eq!(read(dir, AGENTS), customized);
}

#[test]
fn upgrades_and_repairs_persist_ownership_of_new_config_skills_and_instructions() {
    for repair in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        setup(dir);
        let skill = ".agents/skills/next/SKILL.md";
        let expected_skill = read(dir, skill);
        let expected_agents = read(dir, AGENTS);
        let mut manifest = ScaffoldManifest::load(dir);
        write(dir, skill, "An older generated workflow\n");
        manifest.record(skill, "An older generated workflow\n");
        let old_block = format!("{START}\nOlder generated instructions\n{END}");
        write(dir, AGENTS, &old_block);
        manifest
            .codex
            .as_mut()
            .unwrap()
            .agents
            .as_mut()
            .unwrap()
            .generated = old_block;
        let mut config = read(dir, CONFIG).parse::<toml_edit::DocumentMut>().unwrap();
        config["mcp_servers"]["magistr"]["startup_timeout_sec"] = toml_edit::value(5);
        write(dir, CONFIG, &config.to_string());
        // Ownership records serialized bytes, including TOML value decoration.
        let config = read(dir, CONFIG).parse::<toml_edit::DocumentMut>().unwrap();
        manifest
            .codex
            .as_mut()
            .unwrap()
            .config
            .get_mut("magistr.startup_timeout_sec")
            .unwrap()
            .generated = config["mcp_servers"]["magistr"]["startup_timeout_sec"].to_string();
        manifest.save(dir).unwrap();

        if repair {
            let repaired = repair_codex(dir).unwrap();
            for path in [CONFIG, AGENTS, skill] {
                assert!(repaired.iter().any(|value| value == path), "{path}");
            }
        } else {
            setup(dir);
        }
        assert_eq!(read(dir, skill), expected_skill);
        assert_eq!(read(dir, AGENTS), expected_agents);
        let config: toml::Value = toml::from_str(&read(dir, CONFIG)).unwrap();
        assert_eq!(
            config["mcp_servers"]["magistr"]["startup_timeout_sec"].as_integer(),
            Some(60)
        );
        assert!(!ScaffoldManifest::load(dir).is_customized(skill, &expected_skill));
        assert!(repair_codex(dir).unwrap().is_empty());
        CodexSetup::remove_target(dir).unwrap();
        assert!(!dir.join(skill).exists());
        assert!(!read(dir, AGENTS).contains(START));
        let remaining: toml::Value = toml::from_str(&read(dir, CONFIG)).unwrap();
        assert!(
            remaining
                .get("mcp_servers")
                .and_then(|value| value.get("magistr"))
                .is_none()
        );
    }
}

#[test]
fn selective_apply_claims_only_the_approved_codex_component() {
    let skill = ".agents/skills/next/SKILL.md";
    for approved in [CONFIG, AGENTS, skill] {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let preview = crate::scaffold::preview(
            dir,
            &vars(),
            &HashMap::new(),
            &crate::scaffold::SetupSeeds::default(),
        )
        .unwrap();
        let selected = preview
            .files
            .iter()
            .find(|file| file.path == approved)
            .unwrap();
        assert_eq!(
            apply_approved(dir, &[selected]).unwrap().applied_paths(),
            [approved]
        );
        let manifest = ScaffoldManifest::load(dir);
        let ownership = manifest.codex.as_ref().unwrap();
        assert!(ownership.selected);
        assert_eq!(!ownership.config.is_empty(), approved == CONFIG);
        assert_eq!(ownership.agents.is_some(), approved == AGENTS);
        assert_eq!(manifest.files.contains_key(skill), approved == skill);
        for unapproved in [CONFIG, AGENTS, skill]
            .into_iter()
            .filter(|path| *path != approved)
        {
            assert!(!dir.join(unapproved).exists());
            write(
                dir,
                unapproved,
                if unapproved == CONFIG {
                    "model = 'user-owned'\n"
                } else {
                    "User-owned content\n"
                },
            );
        }
        CodexSetup::remove_target(dir).unwrap();
        for unapproved in [CONFIG, AGENTS, skill]
            .into_iter()
            .filter(|path| *path != approved)
        {
            assert_eq!(
                read(dir, unapproved),
                if unapproved == CONFIG {
                    "model = 'user-owned'\n"
                } else {
                    "User-owned content\n"
                }
            );
        }
    }
}

#[test]
fn target_removal_keeps_native_state_until_an_explicit_reset() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);
    write(dir, ".magistr/state.json", "native execution checkpoint\n");
    write(
        dir,
        ".think-and-ship/project.json",
        "native project identity\n",
    );
    let project = read(dir, ".magistr/project.json");
    let claude_skill = read(dir, ".claude/skills/next/SKILL.md");
    CodexSetup::remove_target(dir).unwrap();
    assert_eq!(read(dir, ".magistr/project.json"), project);
    assert_eq!(
        read(dir, ".magistr/state.json"),
        "native execution checkpoint\n"
    );
    assert_eq!(
        read(dir, ".think-and-ship/project.json"),
        "native project identity\n"
    );
    assert_eq!(read(dir, ".claude/skills/next/SKILL.md"), claude_skill);
    assert!(!CodexSetup::is_present(dir));
    let mut defaults = vars();
    defaults.remove("selected_agents");
    let preview = crate::scaffold::preview(
        dir,
        &defaults,
        &HashMap::new(),
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();
    assert!(!preview.files.iter().any(|file| file.path == CONFIG));
    crate::scaffold::remove_scaffold(dir).unwrap();
    assert!(!dir.join(".magistr").exists());
    assert!(!dir.join(".claude/skills/next/SKILL.md").exists());
}

#[test]
fn inline_mcp_tables_keep_user_values_through_setup_and_removal() {
    for inline in [
        "{ custom = { command = 'mine' } }",
        "{ magistr = { command = 'custom-magistr', args = ['user-root'] }, custom = { command = 'mine' } }",
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let original =
            format!("# User inline configuration\nmcp_servers = {inline}\nmodel = 'chosen'\n");
        let before: toml::Value = toml::from_str(&original).unwrap();
        write(dir, CONFIG, &original);
        setup(dir);
        setup(dir);
        let content = read(dir, CONFIG);
        let configured: toml::Value = toml::from_str(&content).expect("generated TOML is valid");
        assert!(content.contains("# User inline configuration"));
        assert_eq!(
            configured["mcp_servers"]["custom"],
            before["mcp_servers"]["custom"]
        );
        assert_eq!(configured["model"], before["model"]);
        if let Some(magistr) = before["mcp_servers"].get("magistr") {
            assert_eq!(&configured["mcp_servers"]["magistr"], magistr);
        }
        CodexSetup::remove_target(dir).unwrap();
        let remaining: toml::Value = toml::from_str(&read(dir, CONFIG)).unwrap();
        assert_eq!(remaining, before);
    }
}

#[test]
fn customizing_either_transport_field_protects_command_and_args_as_one_unit() {
    for server in ["magistr", "workbench-mcp"] {
        for field in ["command", "args"] {
            let tmp = tempfile::tempdir().unwrap();
            let dir = tmp.path();
            setup(dir);
            let mut config = read(dir, CONFIG).parse::<toml_edit::DocumentMut>().unwrap();
            config["mcp_servers"][server][field] = if field == "command" {
                toml_edit::value("custom-wrapper")
            } else {
                toml_edit::value(["custom-root"].into_iter().collect::<toml_edit::Array>())
            };
            write(dir, CONFIG, &config.to_string());
            let customized: toml::Value = toml::from_str(&read(dir, CONFIG)).unwrap();
            let mut upgrade_vars = vars();
            upgrade_vars.insert("project_root".into(), "new-generated-root".into());
            generate_with_result(
                dir,
                &upgrade_vars,
                &HashMap::new(),
                &crate::scaffold::SetupSeeds::default(),
            )
            .unwrap();
            for stage in ["upgrade", "repair", "removal"] {
                match stage {
                    "repair" => {
                        repair_codex(dir).unwrap();
                    }
                    "removal" => {
                        CodexSetup::remove_target(dir).unwrap();
                    }
                    _ => {}
                }
                let current: toml::Value = toml::from_str(&read(dir, CONFIG)).unwrap();
                for key in ["command", "args"] {
                    assert_eq!(
                        current["mcp_servers"][server].get(key),
                        customized["mcp_servers"][server].get(key),
                        "{server} customized {field}, {stage} changed {key}"
                    );
                }
            }
        }
    }
}

#[test]
fn retired_codex_files_need_explicit_target_ownership_before_removal() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);
    let retired = ".agents/skills/retired-magistr-workflow/SKILL.md";
    let unrelated = ".agents/skills/team/SKILL.md";
    let mut manifest = ScaffoldManifest::load(dir);
    for path in [retired, unrelated] {
        write(dir, path, "Previously generated content\n");
        manifest.record(path, "Previously generated content\n");
    }
    manifest
        .codex
        .as_mut()
        .unwrap()
        .files
        .insert(retired.to_owned());
    manifest.save(dir).unwrap();
    let changed = CodexSetup::remove_target(dir).unwrap();
    assert!(changed.iter().any(|path| path == retired));
    assert!(!dir.join(retired).exists());
    assert!(!ScaffoldManifest::load(dir).files.contains_key(retired));
    assert_eq!(read(dir, unrelated), "Previously generated content\n");
    assert!(ScaffoldManifest::load(dir).files.contains_key(unrelated));
}

#[test]
fn metadata_only_server_settings_gain_a_transport_and_survive_target_removal() {
    for server in ["magistr", "workbench-mcp", "think-and-ship"] {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let original = format!(
            "# Existing preferences\n[mcp_servers.{server}]\nstartup_timeout_sec = 120\n[mcp_servers.{server}.env]\nTEAM_SETTING = 'keep'\n"
        );
        write(dir, CONFIG, &original);
        setup(dir);
        let config: toml::Value = toml::from_str(&read(dir, CONFIG)).unwrap();
        let entry = &config["mcp_servers"][server];
        assert_eq!(entry["startup_timeout_sec"].as_integer(), Some(120));
        assert_eq!(entry["env"]["TEAM_SETTING"].as_str(), Some("keep"));
        if server == "think-and-ship" {
            assert_eq!(entry["command"].as_str(), Some("think-and-ship"));
            assert_eq!(
                entry["args"].as_array().unwrap(),
                &[toml::Value::String("serve".into())]
            );
            assert_eq!(entry["enabled"].as_bool(), Some(false));
        } else {
            assert_eq!(entry["command"].as_str(), Some("magistr"));
            let args: Vec<_> = entry["args"]
                .as_array()
                .unwrap()
                .iter()
                .map(|arg| arg.as_str().unwrap())
                .collect();
            assert_eq!(args, ["mcp-launch", server, "."]);
        }
        setup(dir);
        repair_codex(dir).unwrap();
        CodexSetup::remove_target(dir).unwrap();
        assert_eq!(read(dir, CONFIG), original);
    }
}

#[test]
fn a_partial_transport_fails_actionably_before_any_scaffold_write() {
    for server in ["magistr", "workbench-mcp", "think-and-ship"] {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write(
            dir,
            CONFIG,
            &format!("[mcp_servers.{server}]\nargs = ['configured-root']\n"),
        );
        write(dir, AGENTS, "# Existing instructions\n");
        let before = snapshot(dir);
        let preview_error = crate::scaffold::preview(
            dir,
            &vars(),
            &HashMap::new(),
            &crate::scaffold::SetupSeeds::default(),
        )
        .unwrap_err();
        let setup_error = generate_with_result(
            dir,
            &vars(),
            &HashMap::new(),
            &crate::scaffold::SetupSeeds::default(),
        )
        .unwrap_err();
        for error in [preview_error, setup_error] {
            let message = error.to_string();
            assert!(message.contains(server), "{message}");
            assert!(message.contains("no command or URL"), "{message}");
            assert!(message.contains("restore its transport"), "{message}");
        }
        assert_eq!(snapshot(dir), before);
    }
}

#[test]
#[cfg(unix)]
fn an_unchanged_owned_launcher_regains_execute_bits_during_repair_or_reinit() {
    use std::os::unix::fs::PermissionsExt;

    for repair in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        setup(dir);
        let path = dir.join(".codex/run");
        let content = std::fs::read(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        if repair {
            assert!(
                repair_codex(dir)
                    .unwrap()
                    .contains(&".codex/run".to_owned())
            );
        } else {
            setup(dir);
        }
        assert_eq!(std::fs::read(&path).unwrap(), content);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o755
        );
        assert!(repair_codex(dir).unwrap().is_empty());

        write(dir, ".codex/run", "#!/bin/sh\n# User-managed wrapper\n");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        if repair {
            repair_codex(dir).unwrap();
        } else {
            setup(dir);
        }
        assert_eq!(
            read(dir, ".codex/run"),
            "#!/bin/sh\n# User-managed wrapper\n"
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn default_preview_rejects_corrupted_codex_ownership_before_selection_recovery() {
    for malformed in ["{broken", r#"{"files":7,"codex":{"selected":true}}"#] {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        setup(dir);
        write(dir, ".magistr/scaffold-manifest.json", malformed);
        let before = snapshot(dir);
        let mut defaults = vars();
        defaults.remove("selected_agents");
        let result = crate::scaffold::preview(
            dir,
            &defaults,
            &HashMap::new(),
            &crate::scaffold::SetupSeeds::default(),
        );
        let unchanged = snapshot(dir) == before;
        assert!(
            result.is_err() && unchanged,
            "Default preview must reject broken ownership evidence without changing any bytes: refused={}, unchanged={unchanged}",
            result.is_err()
        );
    }
}

#[test]
fn default_reinit_rejects_corrupted_codex_ownership_before_selection_recovery() {
    for malformed in ["{broken", r#"{"files":7,"codex":{"selected":true}}"#] {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        setup(dir);
        write(dir, ".magistr/scaffold-manifest.json", malformed);
        let before = snapshot(dir);
        let mut defaults = vars();
        defaults.remove("selected_agents");
        let result = generate_with_result(
            dir,
            &defaults,
            &HashMap::new(),
            &crate::scaffold::SetupSeeds::default(),
        );
        let unchanged = snapshot(dir) == before;
        assert!(
            result.is_err() && unchanged,
            "Default reinit must reject broken ownership evidence without changing any bytes: refused={}, unchanged={unchanged}",
            result.is_err()
        );
    }
}

/// The removal plan says what the owned removal does before it does it:
/// recorded, unchanged skills and the launcher are removals, a customized
/// skill is preserved with its reason, the shared components are restored
/// merges, and native project state is never in the plan.
#[test]
fn the_removal_plan_lists_removals_preserved_customizations_and_shared_restores() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    setup(dir);
    write(
        dir,
        ".agents/skills/next/SKILL.md",
        "# the team's own next\n",
    );

    let plan = CodexSetup::removal_plan(dir).unwrap();
    assert_eq!(plan.operation, crate::scaffold::AdminOperation::Removal);
    let customized = plan.get(".agents/skills/next/SKILL.md").unwrap();
    assert_eq!(customized.action, FileAction::Preserved);
    assert_eq!(customized.ownership, crate::scaffold::Ownership::Customized);
    let launcher = plan.get(".codex/run").unwrap();
    assert_eq!(launcher.action, FileAction::Remove);
    assert_eq!(launcher.ownership, crate::scaffold::Ownership::Generated);
    assert_eq!(
        launcher.expected(),
        crate::scaffold::Expected::of(Some(&read(dir, ".codex/run")))
    );
    let validate = plan.get(".agents/skills/validate/SKILL.md").unwrap();
    assert_eq!(validate.action, FileAction::Remove);
    let config = plan.get(".codex/config.toml").unwrap();
    assert_eq!(config.ownership, crate::scaffold::Ownership::Shared);
    assert!(plan.get(".magistr/project.json").is_none());
    assert!(plan.get(".think-and-ship/project.json").is_none());
    assert!(plan.get("CLAUDE.md").is_none());

    let removed = CodexSetup::remove_target(dir).unwrap();
    assert!(removed.contains(&".codex/run".to_owned()));
    assert!(!removed.contains(&".agents/skills/next/SKILL.md".to_owned()));
    assert_eq!(
        read(dir, ".agents/skills/next/SKILL.md"),
        "# the team's own next\n"
    );
    assert!(!dir.join(".agents/skills/validate").exists());
    assert!(dir.join(".magistr/project.json").is_file());
}
