#[cfg(unix)]
use crate::scaffold::{CodexSetup, FileAction, apply::apply_approved, manifest::ScaffoldManifest};
use crate::scaffold::{generate_with_result, pipeline::ScaffoldPipeline, preview};
use std::collections::{BTreeMap, HashMap};
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

const SKILLS: &[&str] = &[
    "craft",
    "init",
    "next",
    "next-loop",
    "roadmap",
    "roadmap-refresh",
    "signals",
    "validate",
    "workbench-routing",
];

fn vars() -> HashMap<String, String> {
    ScaffoldPipeline::headless(crate::types::Stack::default())
        .with_agents(&["codex".to_owned()])
        .template_vars()
}

fn write(dir: &Path, relative: &str, content: &str) {
    let path = dir.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

#[cfg(unix)]
fn link(dir: &Path, name: &str, content: Option<&str>) {
    if let Some(content) = content {
        write(dir, &format!(".claude/skills/{name}/SKILL.md"), content);
    }
    std::fs::create_dir_all(dir.join(".agents/skills")).unwrap();
    symlink(
        format!("../../.claude/skills/{name}"),
        dir.join(format!(".agents/skills/{name}")),
    )
    .unwrap();
}

#[cfg(unix)]
fn legacy_skills(dir: &Path) {
    for name in SKILLS {
        link(dir, name, Some(&format!("Legacy Claude {name} workflow\n")));
    }
    write(
        dir,
        ".claude/skills/craft/references/runtime.md",
        "Legacy Claude runtime adapter\n",
    );
}

#[derive(Debug, PartialEq, Eq)]
enum Entry {
    Directory,
    File(Vec<u8>),
    Symlink(PathBuf),
}

fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Entry> {
    fn visit(root: &Path, dir: &Path, entries: &mut BTreeMap<PathBuf, Entry>) {
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

#[cfg(unix)]
#[test]
fn nine_legacy_skill_links_preview_without_writes_and_generate_native_codex_files() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    legacy_skills(dir);
    write(
        dir,
        ".agents/skills/team/SKILL.md",
        "User's team workflow\n",
    );
    let before = snapshot(dir);
    let planned = preview(dir, &vars(), &HashMap::new()).unwrap();
    assert_eq!(snapshot(dir), before);
    let skills: Vec<_> = planned
        .files
        .iter()
        .filter(|file| file.path.starts_with(".agents/skills/"))
        .collect();
    assert_eq!(skills.len(), 10);
    for file in &skills {
        assert_eq!(file.action, FileAction::Overwrite, "{}", file.path);
        assert!(
            file.diff
                .as_deref()
                .unwrap()
                .contains("legacy skill symlink"),
            "{}",
            file.path
        );
    }
    generate_with_result(dir, &vars(), &HashMap::new()).unwrap();
    for name in SKILLS {
        assert!(
            std::fs::symlink_metadata(dir.join(format!(".agents/skills/{name}")))
                .unwrap()
                .is_dir(),
            "{name} remained a symlink"
        );
    }
    let manifest = ScaffoldManifest::load(dir);
    for file in skills {
        assert_eq!(
            std::fs::read_to_string(dir.join(&file.path)).unwrap(),
            file.new_content
        );
        assert!(manifest.codex.as_ref().unwrap().files.contains(&file.path));
        assert!(!manifest.is_customized(&file.path, &file.new_content));
    }
    // Full generation also updates Claude's own adapter through its emitter.
    for file in planned
        .files
        .iter()
        .filter(|file| file.path.starts_with(".claude/skills/"))
    {
        assert_eq!(
            std::fs::read_to_string(dir.join(&file.path)).unwrap(),
            file.new_content,
            "Codex migration wrote through to {}",
            file.path
        );
    }
    assert_eq!(
        std::fs::read_to_string(dir.join(".agents/skills/team/SKILL.md")).unwrap(),
        "User's team workflow\n"
    );
}

#[cfg(unix)]
#[test]
fn selected_link_migration_preserves_claude_sources_and_claims_only_approved_files() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    legacy_skills(dir);
    let claude_before = snapshot(&dir.join(".claude"));
    let planned = preview(dir, &vars(), &HashMap::new()).unwrap();
    let path = ".agents/skills/next/SKILL.md";
    let selected = planned.files.iter().find(|file| file.path == path).unwrap();
    assert_eq!(apply_approved(dir, &[selected]).unwrap(), [path]);
    assert_eq!(snapshot(&dir.join(".claude")), claude_before);
    assert!(
        std::fs::symlink_metadata(dir.join(".agents/skills/next"))
            .unwrap()
            .is_dir()
    );
    let manifest = ScaffoldManifest::load(dir);
    assert_eq!(manifest.codex.as_ref().unwrap().files.len(), 1);
    assert!(manifest.codex.as_ref().unwrap().files.contains(path));
    assert!(!dir.join(".codex/config.toml").exists());
    for name in SKILLS.iter().filter(|name| **name != "next") {
        assert!(
            std::fs::symlink_metadata(dir.join(format!(".agents/skills/{name}")))
                .unwrap()
                .is_symlink()
        );
    }
    CodexSetup::remove_target(dir).unwrap();
    assert!(!dir.join(path).exists());
    assert_eq!(snapshot(&dir.join(".claude")), claude_before);
}

#[cfg(unix)]
#[test]
fn craft_link_migration_requires_skill_and_runtime_companion_approval_together() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    legacy_skills(dir);
    let planned = preview(dir, &vars(), &HashMap::new()).unwrap();
    let craft: Vec<_> = planned
        .files
        .iter()
        .filter(|file| file.path.starts_with(".agents/skills/craft/"))
        .collect();
    assert_eq!(craft.len(), 2);
    let sentinel = planned
        .files
        .iter()
        .find(|file| file.path == ".codex/run")
        .unwrap();
    let before = snapshot(dir);
    for child in &craft {
        let error = apply_approved(dir, &[sentinel, child])
            .unwrap_err()
            .to_string();
        assert!(error.contains("requires selecting"), "{error}");
        assert!(error.contains("runtime.md"), "{error}");
        assert_eq!(snapshot(dir), before);
    }
    let claude_before = snapshot(&dir.join(".claude"));
    assert_eq!(apply_approved(dir, &craft).unwrap().len(), 2);
    assert_eq!(snapshot(&dir.join(".claude")), claude_before);
    for file in craft {
        assert_eq!(
            std::fs::read_to_string(dir.join(&file.path)).unwrap(),
            file.new_content
        );
    }
}

#[cfg(unix)]
#[test]
fn dangling_legacy_link_migrates_without_creating_a_claude_source() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    link(dir, "next", None);
    let before = snapshot(dir);
    let planned = preview(dir, &vars(), &HashMap::new()).unwrap();
    assert_eq!(snapshot(dir), before);
    let selected = planned
        .files
        .iter()
        .find(|file| file.path == ".agents/skills/next/SKILL.md")
        .unwrap();
    assert_eq!(selected.action, FileAction::Overwrite);
    apply_approved(dir, &[selected]).unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.join(&selected.path)).unwrap(),
        selected.new_content
    );
    assert!(!dir.join(".claude").exists());
}

#[cfg(unix)]
#[test]
fn equal_content_still_previews_and_applies_legacy_link_replacement() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let clean = preview(dir, &vars(), &HashMap::new()).unwrap();
    let expected = clean
        .files
        .iter()
        .find(|file| file.path == ".agents/skills/next/SKILL.md")
        .unwrap()
        .new_content
        .clone();
    link(dir, "next", Some(&expected));
    let planned = preview(dir, &vars(), &HashMap::new()).unwrap();
    let selected = planned
        .files
        .iter()
        .find(|file| file.path == ".agents/skills/next/SKILL.md")
        .unwrap();
    assert_eq!(selected.action, FileAction::Overwrite);
    assert!(
        selected
            .diff
            .as_ref()
            .unwrap()
            .contains("legacy skill symlink")
    );
    apply_approved(dir, &[selected]).unwrap();
    assert!(
        std::fs::symlink_metadata(dir.join(".agents/skills/next"))
            .unwrap()
            .is_dir()
    );
    assert_eq!(
        std::fs::read_to_string(dir.join(".claude/skills/next/SKILL.md")).unwrap(),
        expected
    );
}

#[cfg(unix)]
#[test]
fn retargeted_or_replaced_legacy_links_reject_selected_batch_even_when_text_matches() {
    for replace_with_directory in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let content = "Same existing text\n";
        link(dir, "next", Some(content));
        let planned = preview(dir, &vars(), &HashMap::new()).unwrap();
        let selected = planned
            .files
            .iter()
            .find(|file| file.path == ".agents/skills/next/SKILL.md")
            .unwrap();
        let sentinel = planned
            .files
            .iter()
            .find(|file| file.path == ".codex/run")
            .unwrap();
        let path = dir.join(".agents/skills/next");
        std::fs::remove_file(&path).unwrap();
        if replace_with_directory {
            write(dir, ".agents/skills/next/SKILL.md", content);
        } else {
            write(outside.path(), "SKILL.md", content);
            symlink(outside.path(), path).unwrap();
        }
        let before = snapshot(dir);
        let external_before = snapshot(outside.path());
        assert!(apply_approved(dir, &[sentinel, selected]).is_err());
        assert_eq!(snapshot(dir), before);
        assert_eq!(snapshot(outside.path()), external_before);
    }
}

#[cfg(unix)]
#[test]
fn malformed_config_rejects_setup_before_detaching_any_legacy_skill_link() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    legacy_skills(dir);
    write(
        dir,
        ".codex/config.toml",
        "[mcp_servers.magistr\ncommand = 'broken'\n",
    );
    let before = snapshot(dir);
    assert!(preview(dir, &vars(), &HashMap::new()).is_err());
    assert_eq!(snapshot(dir), before);
    assert!(generate_with_result(dir, &vars(), &HashMap::new()).is_err());
    assert_eq!(snapshot(dir), before);
}

#[cfg(unix)]
#[test]
fn unsafe_link_among_recognized_skills_rejects_the_entire_migration_without_writes() {
    for unsafe_source_ancestor in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        legacy_skills(dir);
        if unsafe_source_ancestor {
            let external = outside.path().join("claude");
            std::fs::rename(dir.join(".claude"), &external).unwrap();
            symlink(external, dir.join(".claude")).unwrap();
        } else {
            let path = dir.join(".agents/skills/workbench-routing");
            std::fs::remove_file(&path).unwrap();
            write(outside.path(), "SKILL.md", "External workflow\n");
            symlink(outside.path(), path).unwrap();
        }
        let before = snapshot(dir);
        let external_before = snapshot(outside.path());
        assert!(preview(dir, &vars(), &HashMap::new()).is_err());
        assert!(generate_with_result(dir, &vars(), &HashMap::new()).is_err());
        assert_eq!(snapshot(dir), before);
        assert_eq!(snapshot(outside.path()), external_before);
    }
}

#[cfg(unix)]
#[test]
fn startup_repair_leaves_legacy_link_migration_to_explicit_initialization() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    generate_with_result(dir, &vars(), &HashMap::new()).unwrap();
    std::fs::remove_file(dir.join(".agents/skills/next/SKILL.md")).unwrap();
    std::fs::remove_dir(dir.join(".agents/skills/next")).unwrap();
    link(dir, "next", None);
    let before = snapshot(dir);
    assert!(CodexSetup::repair(dir).unwrap().is_empty());
    assert_eq!(snapshot(dir), before);
}

#[test]
fn native_skill_directories_are_not_legacy_links_and_survive_reinitialization() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    generate_with_result(dir, &vars(), &HashMap::new()).unwrap();
    write(
        dir,
        ".agents/skills/team/SKILL.md",
        "User's team workflow\n",
    );
    for name in SKILLS {
        let metadata =
            std::fs::symlink_metadata(dir.join(format!(".agents/skills/{name}"))).unwrap();
        assert!(metadata.is_dir() && !metadata.is_symlink(), "{name}");
    }
    let before = snapshot(&dir.join(".agents/skills"));
    let planned = preview(dir, &vars(), &HashMap::new()).unwrap();
    assert_eq!(snapshot(&dir.join(".agents/skills")), before);
    for file in planned
        .files
        .iter()
        .filter(|file| file.path.starts_with(".agents/skills/"))
    {
        assert!(
            !file
                .diff
                .as_deref()
                .unwrap_or_default()
                .contains("legacy skill symlink"),
            "{} was treated as a legacy link",
            file.path
        );
    }
    generate_with_result(dir, &vars(), &HashMap::new()).unwrap();
    assert_eq!(snapshot(&dir.join(".agents/skills")), before);
}
