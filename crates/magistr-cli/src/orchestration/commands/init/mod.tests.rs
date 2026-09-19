use super::*;
use crate::scaffold::drift::DriftReport;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A magistr project whose recorded workflow runtime the user deleted: the
/// startup repair preserves the runtime (not recreated) and refuses the
/// workflow skills that depend on it.
fn project_with_deleted_runtime() -> tempfile::TempDir {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    std::fs::create_dir_all(dir.join(".magistr")).unwrap();
    std::fs::write(dir.join(".magistr/project.json"), "{}").unwrap();
    let skill = ".claude/skills/next/SKILL.md";
    let old = "# A workflow generated before shared runtime references\n";
    std::fs::create_dir_all(dir.join(".claude/skills/next")).unwrap();
    std::fs::write(dir.join(skill), old).unwrap();
    let mut manifest = scaffold::manifest::ScaffoldManifest::new();
    manifest.record(skill, old);
    manifest.record(RUNTIME, "# an earlier runtime\n");
    manifest.save(dir).unwrap();
    tmp
}

const RUNTIME: &str = ".claude/skills/craft/references/runtime.md";

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

fn clean_drift() -> DriftReport {
    DriftReport {
        new_projects: Vec::new(),
        removed_archetypes: Vec::new(),
        changes: Vec::new(),
    }
}

/// The check report carries the drift fields the tool always answered and
/// the startup repair plan in the planned-change shape: a refused skill is
/// visible with its reason before the next mcp-serve applies the plan, and
/// computing the report writes nothing.
#[test]
fn the_check_report_shows_the_repair_plan_with_its_refusals_without_writing() {
    let tmp = project_with_deleted_runtime();
    let before = files(tmp.path());

    let repair = scaffold::repair::plan(tmp.path());
    let report = check_report_json(&clean_drift(), &repair, PlanDetail::Summary);

    assert_eq!(report["clean"], true);
    assert_eq!(report["total_changes"], 0);
    assert_eq!(report["repair"]["operation"], "repair");
    assert_eq!(report["repair"]["summary"]["refused"], 1);
    assert_eq!(report["repair"]["summary"]["preserved"], 1);
    let entries = report["repair"]["files"].as_array().unwrap();
    let skill = entries
        .iter()
        .find(|e| e["path"] == ".claude/skills/next/SKILL.md")
        .unwrap();
    assert_eq!(skill["action"], "refused");
    assert_eq!(skill["intent"], "refuse");
    assert_eq!(skill["ownership"], "internal");
    assert!(
        skill["reason"]
            .as_str()
            .unwrap()
            .contains("depends on .claude/skills/craft/references/runtime.md"),
        "{skill}"
    );
    let runtime = entries.iter().find(|e| e["path"] == RUNTIME).unwrap();
    assert_eq!(runtime["action"], "preserved");
    assert_eq!(runtime["expected"], "absent");
    assert_eq!(runtime["reason"], "removed by the user; not recreated");

    assert_eq!(files(tmp.path()), before, "a preview writes nothing");
}

/// The summary detail leaves the diff to the CLI; the full detail keeps it.
#[test]
fn plan_detail_decides_whether_an_overwrite_carries_its_diff() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    std::fs::create_dir_all(dir.join(".magistr")).unwrap();
    std::fs::write(dir.join(".magistr/project.json"), "{}").unwrap();
    std::fs::create_dir_all(dir.join(".claude/rules")).unwrap();
    std::fs::write(
        dir.join(".claude/rules/tools.md"),
        "# an older magistr wrote this\n",
    )
    .unwrap();
    let repair = scaffold::repair::plan(dir);
    assert_eq!(repair.overwrites(), 1);

    let full = plan_json(&repair, PlanDetail::Full);
    let summary = plan_json(&repair, PlanDetail::Summary);
    let guide = |value: &serde_json::Value| {
        value["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["path"] == ".claude/rules/tools.md")
            .cloned()
            .unwrap()
    };
    assert_eq!(guide(&full)["action"], "overwrite");
    assert!(guide(&full)["diff"].is_string());
    assert!(guide(&summary)["diff"].is_null());
    assert_eq!(full["summary"]["overwrite"], 1);
    assert_eq!(summary["summary"], full["summary"]);
}
