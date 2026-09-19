use super::*;
use crate::output::{JsonRenderer, SilentRenderer};
use crate::scaffold::FileAction;
use crate::scaffold::manifest::ScaffoldManifest;
use crate::scaffold::pipeline::ScaffoldPipeline;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A project with the managed Codex target set up and one skill customized.
fn codex_project() -> tempfile::TempDir {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    let inputs = crate::config::inputs::ProjectInputs::from_lookup(dir, |_| None);
    ScaffoldPipeline::headless(crate::types::Stack::default())
        .with_inputs(&inputs)
        .with_agents(&["codex".to_owned()])
        .generate(dir)
        .unwrap();
    std::fs::write(
        dir.join(".agents/skills/next/SKILL.md"),
        "# the team's own next\n",
    )
    .unwrap();
    tmp
}

fn files(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(root, &path, out);
            } else {
                out.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    std::fs::read(&path).unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    visit(dir, dir, &mut out);
    out
}

/// The dry run prints the removal plan and writes nothing: the launcher,
/// the recorded skills and the shared components are still there, the
/// manifest still records the Codex target, and the plan it would have
/// applied still lists the launcher as a removal and the customized skill
/// as preserved afterwards.
#[test]
fn a_dry_run_leaves_the_codex_target_and_its_record_in_place() {
    let tmp = codex_project();
    let dir = tmp.path();
    let before = files(dir);
    assert!(
        ScaffoldManifest::load(dir)
            .codex
            .is_some_and(|o| o.selected)
    );

    cmd_codex_remove(dir, true, &JsonRenderer::new()).unwrap();
    assert_eq!(files(dir), before, "the JSON dry run writes nothing");
    cmd_codex_remove(dir, true, &SilentRenderer::new()).unwrap();
    assert_eq!(files(dir), before, "the plain dry run writes nothing");

    let plan = crate::scaffold::CodexSetup::removal_plan(dir).unwrap();
    assert_eq!(plan.get(".codex/run").unwrap().action, FileAction::Remove);
    assert_eq!(
        plan.get(".agents/skills/next/SKILL.md").unwrap().action,
        FileAction::Preserved
    );
    assert!(
        ScaffoldManifest::load(dir)
            .codex
            .is_some_and(|o| o.selected)
    );
}

/// Without the flag the removal applies, as before: the launcher goes,
/// the customized skill stays, and the record is cleared.
#[test]
fn without_the_flag_the_removal_applies() {
    let tmp = codex_project();
    let dir = tmp.path();
    cmd_codex_remove(dir, false, &SilentRenderer::new()).unwrap();
    assert!(!dir.join(".codex/run").exists());
    assert_eq!(
        std::fs::read_to_string(dir.join(".agents/skills/next/SKILL.md")).unwrap(),
        "# the team's own next\n"
    );
    assert!(ScaffoldManifest::load(dir).codex.is_none());
    assert!(dir.join(".magistr/project.json").is_file());
}

/// A project with no managed Codex target has an empty plan to show and
/// nothing to remove; neither rendering fails.
#[test]
fn a_project_without_a_codex_target_previews_an_empty_plan() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    let inputs = crate::config::inputs::ProjectInputs::from_lookup(dir, |_| None);
    ScaffoldPipeline::headless(crate::types::Stack::default())
        .with_inputs(&inputs)
        .generate(dir)
        .unwrap();
    let before = files(dir);
    cmd_codex_remove(dir, true, &JsonRenderer::new()).unwrap();
    cmd_codex_remove(dir, true, &SilentRenderer::new()).unwrap();
    assert_eq!(files(dir), before);
}
