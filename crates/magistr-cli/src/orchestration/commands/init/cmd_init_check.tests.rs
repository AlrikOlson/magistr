use super::*;
use crate::output::JsonRenderer;
use crate::scaffold::pipeline::ScaffoldPipeline;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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

/// `magistr init --check` is read-only in both renderings: over a project
/// with a stale owned file and a deleted runtime the repair plan has an
/// overwrite, a preserved entry and a refusal to show, and the tree is the
/// same afterwards.
#[test]
fn check_reports_the_pending_repair_without_writing() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    let inputs = crate::config::inputs::ProjectInputs::from_lookup(dir, |_| None);
    ScaffoldPipeline::headless(crate::types::Stack::default())
        .with_inputs(&inputs)
        .generate(dir)
        .unwrap();
    std::fs::write(
        dir.join(".claude/rules/tools.md"),
        "# an older magistr wrote this\n",
    )
    .unwrap();
    std::fs::remove_file(dir.join(".claude/skills/craft/references/runtime.md")).unwrap();
    let before = files(dir);

    let repair = scaffold::repair::plan(dir);
    assert_eq!(repair.overwrites(), 1, "{repair:?}");
    assert!(repair.refusals() >= 1, "{repair:?}");
    assert_eq!(repair.preserved(), 1, "{repair:?}");

    cmd_init_check(dir, &JsonRenderer::new()).unwrap();
    assert_eq!(files(dir), before, "the JSON check writes nothing");
    cmd_init_check(dir, &crate::output::SilentRenderer::new()).unwrap();
    assert_eq!(files(dir), before, "the plain check writes nothing");
}

/// A directory that was never initialized is an error naming the missing
/// config, not an empty report.
#[test]
fn check_refuses_a_project_without_config() {
    let tmp = tempfile::TempDir::new().unwrap();
    let error = cmd_init_check(tmp.path(), &JsonRenderer::new()).unwrap_err();
    assert!(error.to_string().contains("project.json"), "{error}");
}
