use super::*;

use tempfile::TempDir;

/// A project root with the marker that makes it a magistr project.
fn project() -> TempDir {
    let tmp = TempDir::new().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join(".magistr")).expect("mkdir");
    std::fs::write(tmp.path().join(".magistr/project.json"), "{}").expect("marker");
    tmp
}

fn owned(path: &str) -> String {
    owned_files()
        .into_iter()
        .find(|(p, _)| *p == path)
        .map(|(_, c)| c)
        .unwrap_or_else(|| panic!("{path} is not an owned file"))
}

/// The set is not empty and carries the two files this exists for: the tool
/// guide (a rule lands there and reaches nobody) and the guard hook.
#[test]
fn the_owned_set_is_the_files_magistr_writes_for_every_project_alike() {
    let paths: Vec<&str> = owned_files().into_iter().map(|(p, _)| p).collect();
    assert!(
        paths.contains(&".claude/rules/tools.md"),
        "the tool guide is owned: {paths:?}"
    );
    assert!(
        paths.contains(&".claude/hooks/magistr-guard.sh"),
        "the guard hook is owned: {paths:?}"
    );
}

/// Rule 1: a file under a policy that is not `Overwrite` is the user's, and
/// a repair must never be able to reach one. `CLAUDE.md` is the sharpest
/// case — `NeverOverwrite`, and full of the user's own prose.
#[test]
fn nothing_the_user_owns_is_in_the_repairable_set() {
    let owned: Vec<&str> = owned_files().into_iter().map(|(p, _)| p).collect();
    for theirs in super::super::scaffold_files(&HashMap::new())
        .into_iter()
        .filter(|f| f.policy != UpdatePolicy::Overwrite)
    {
        assert!(
            !owned.contains(&theirs.path),
            "{} is not magistr's to rewrite",
            theirs.path
        );
    }
    assert!(!owned.contains(&"CLAUDE.md"));
    assert!(!owned.contains(&".claude/rules/conventions.md"));
}

/// Rule 2, and the bug that taught it: `.cursor/rules/conventions.mdc` is
/// *computed* from `language` and comes out fully substituted, so a probe
/// that only looked for a leftover `{{placeholder}}` called it static — and
/// a repair then replaced a Rust project's conventions with the generic
/// default. Nothing derived from the interview may be in the set.
#[test]
fn nothing_computed_from_the_interview_is_in_the_repairable_set() {
    let owned: Vec<&str> = owned_files().into_iter().map(|(p, _)| p).collect();
    for conventions in [
        ".cursor/rules/conventions.mdc",
        ".clinerules",
        ".windsurfrules",
        ".github/copilot-instructions.md",
        ".aider.conf.yml",
    ] {
        assert!(
            !owned.contains(&conventions),
            "{conventions} is computed from the project"
        );
    }
}

/// The probe is only as good as the keys it varies: a key the scaffold reads
/// and this does not set would let a project-dependent file through.
#[test]
fn every_template_var_key_is_probed() {
    let vars = include_str!("../orchestration/commands/misc/build_template_vars.rs");
    for line in vars.lines() {
        let Some(rest) = line.trim().strip_prefix("(\"") else {
            continue;
        };
        let Some(key) = rest.split('"').next() else {
            continue;
        };
        assert!(
            VAR_KEYS.contains(&key),
            "`{key}` is a template variable the repair probe does not vary"
        );
    }
}

/// The whole point: a project whose generated tool guide predates the fix
/// gets it back, without anybody running `magistr init`.
#[test]
fn a_stale_owned_file_is_brought_back_up_to_date() {
    let tmp = project();
    let path = tmp.path().join(".claude/rules/tools.md");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&path, "# an older magistr wrote this\n").expect("write");

    let repaired = repair(tmp.path());
    assert!(
        repaired.contains(&".claude/rules/tools.md".to_owned()),
        "{repaired:?}"
    );
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        owned(".claude/rules/tools.md")
    );
    // And a repaired project is quiet: the second pass writes nothing.
    assert!(repair(tmp.path()).is_empty());
}

#[test]
fn repair_updates_reasoning_guidance_and_preserves_customized_files() {
    let guidance = magistr_engine::prompt::PromptConfig::REASONING_GUIDANCE;
    for customize in [false, true] {
        let tmp = project();
        let vars =
            super::super::pipeline::ScaffoldPipeline::headless(crate::types::Stack::default())
                .with_agents(&["codex".to_owned(), "claude".to_owned()])
                .template_vars();
        super::super::generate_with_result(tmp.path(), &vars, &HashMap::new()).unwrap();
        let mut manifest = super::super::manifest::ScaffoldManifest::load(tmp.path());
        let runtimes = WorkflowSkills::codex_files()
            .into_iter()
            .chain(WorkflowSkills::claude_files())
            .filter(|file| file.path.ends_with("runtime.md"))
            .collect::<Vec<_>>();
        assert_eq!(runtimes.len(), 2);
        let mut expected = Vec::new();
        for file in &runtimes {
            assert!(file.content.contains(guidance));
            let old = file.content.replace(
                guidance,
                "Record reasoning at the beginning and end of an iteration.",
            );
            manifest.record(file.path, &old);
            let current = if customize {
                format!("{old}\nProject-specific reasoning policy.\n")
            } else {
                old
            };
            std::fs::write(tmp.path().join(file.path), &current).unwrap();
            expected.push(if customize {
                current
            } else {
                file.content.clone()
            });
        }
        let skill = ".agents/skills/next/SKILL.md";
        let custom_skill = "# The team's customized implementation workflow\n";
        std::fs::write(tmp.path().join(skill), custom_skill).unwrap();
        let rules = ".claude/rules/conventions.md";
        let custom_rules = "# The team's Rust conventions\n";
        std::fs::write(tmp.path().join(rules), custom_rules).unwrap();
        manifest.save(tmp.path()).unwrap();
        let guide = ".claude/rules/tools.md";
        std::fs::write(tmp.path().join(guide), "# Older generated tool guide\n").unwrap();
        let claude_before = std::fs::read(tmp.path().join("CLAUDE.md")).unwrap();

        let repaired = repair(tmp.path());
        assert!(repaired.contains(&guide.to_owned()), "{repaired:?}");
        assert_eq!(
            std::fs::read_to_string(tmp.path().join(guide)).unwrap(),
            super::super::tool_guide::build_tool_guide(),
        );
        for (file, expected) in runtimes.iter().zip(expected) {
            assert_eq!(
                std::fs::read_to_string(tmp.path().join(file.path)).unwrap(),
                expected,
                "{} customize={customize}",
                file.path,
            );
            assert_eq!(repaired.contains(&file.path.to_owned()), !customize);
        }
        assert_eq!(
            std::fs::read_to_string(tmp.path().join(skill)).unwrap(),
            custom_skill
        );
        assert_eq!(
            std::fs::read_to_string(tmp.path().join(rules)).unwrap(),
            custom_rules
        );
        assert_eq!(
            std::fs::read(tmp.path().join("CLAUDE.md")).unwrap(),
            claude_before
        );
        assert!(repair(tmp.path()).is_empty());
    }
}

#[test]
fn repair_updates_iteration_recovery_but_preserves_customized_codex_skills() {
    let guidance = magistr_engine::prompt::PromptConfig::NEXT_RECOVERY_GUIDANCE.trim_end();
    for customize in [false, true] {
        let tmp = project();
        let vars =
            super::super::pipeline::ScaffoldPipeline::headless(crate::types::Stack::default())
                .with_agents(&["codex".to_owned(), "claude".to_owned()])
                .template_vars();
        super::super::generate_with_result(tmp.path(), &vars, &HashMap::new()).unwrap();
        let skills = WorkflowSkills::codex_files()
            .into_iter()
            .chain(WorkflowSkills::claude_files())
            .filter(|file| file.path.ends_with("/next/SKILL.md"))
            .collect::<Vec<_>>();
        assert_eq!(skills.len(), 2);
        let mut manifest = super::super::manifest::ScaffoldManifest::load(tmp.path());
        let mut expected = Vec::new();
        for skill in &skills {
            let old = skill.content.replace(
                guidance,
                "Read roadmap_next, start its ready chunk, and create an objective.",
            );
            assert_ne!(old, skill.content);
            manifest.record(skill.path, &old);
            let preserve = customize && skill.path.starts_with(".agents/");
            let current = if preserve {
                format!("{old}\nProject-specific recovery policy.\n")
            } else {
                old
            };
            std::fs::write(tmp.path().join(skill.path), &current).unwrap();
            expected.push((
                preserve,
                if preserve {
                    current
                } else {
                    skill.content.clone()
                },
            ));
        }
        manifest.save(tmp.path()).unwrap();

        let repaired = repair(tmp.path());
        for (skill, (preserve, expected)) in skills.iter().zip(expected) {
            assert_eq!(
                std::fs::read_to_string(tmp.path().join(skill.path)).unwrap(),
                expected,
                "{} customize={customize}",
                skill.path,
            );
            assert_eq!(repaired.contains(&skill.path.to_owned()), !preserve);
        }
        assert!(repair(tmp.path()).is_empty());
    }
}

#[test]
fn custom_magistr_gitignore_survives_startup_repair() {
    let tmp = project();
    let relative = ".magistr/.gitignore";
    let path = tmp.path().join(relative);
    let custom = "*\n!.gitignore\n!project.json\n!lint-structure-allowlist.txt\n";
    let mut manifest = super::super::manifest::ScaffoldManifest::new();
    manifest.record(relative, super::super::STATIC_MAGISTR_DIR_GITIGNORE);
    manifest.save(tmp.path()).unwrap();
    std::fs::write(&path, custom).unwrap();

    for _ in 0..2 {
        let repaired = repair(tmp.path());
        assert!(!repaired.iter().any(|item| item == relative));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), custom);
    }
}

/// Rule 3: repair, never create. A file the user deleted stays deleted —
/// scaffolding a project that did not ask for it is init's job, not this.
#[test]
fn a_file_that_is_not_there_is_not_created() {
    let tmp = project();
    assert!(repair(tmp.path()).is_empty());
    assert!(!tmp.path().join(".claude/rules/tools.md").exists());
    assert!(!tmp.path().join("workbench.toml").exists());
}

/// Rule 4: a directory that is not a magistr project is left completely
/// alone, whatever happens to be lying in it.
#[test]
fn a_directory_that_is_not_a_magistr_project_is_untouched() {
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join(".claude/rules/tools.md");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&path, "not mine\n").expect("write");
    assert!(repair(tmp.path()).is_empty());
    assert_eq!(std::fs::read_to_string(&path).expect("read"), "not mine\n");
}

/// The `workbench.toml` half rides along: a project scaffolded before the
/// unattended confirm mode existed has a perfectly good `[commands]` table
/// and no idea a confirm-gated write is about to ask a human who is not
/// there. It is repaired in place, and only once.
#[test]
fn a_workbench_toml_with_no_confirm_mode_is_given_one() {
    let tmp = project();
    let path = tmp.path().join("workbench.toml");
    std::fs::write(&path, "[commands]\n\"test\" = \"cargo test\"\n").expect("write");

    assert!(repair(tmp.path()).contains(&"workbench.toml".to_owned()));
    let body = std::fs::read_to_string(&path).expect("read");
    assert!(body.contains("confirm_mode = \"token\""), "{body}");
    assert!(
        body.starts_with("[commands]\n\"test\" = \"cargo test\"\n"),
        "the human's table is untouched: {body}"
    );
    assert!(repair(tmp.path()).is_empty(), "and only once");
}

/// The canonical-repo rule: in the repository a vendored skill was vendored
/// FROM, the canonical file's own bytes are the truth — a repair must bring
/// the dogfood copy to those bytes (no provenance marker), never back to the
/// embedded version. Without this, every `mcp-serve` startup in workbench-mcp
/// re-appended the marker its lint rule R6 forbids.
#[test]
fn in_the_canonical_repo_a_repair_converges_on_the_canonical_bytes() {
    let tmp = project();
    let canonical = "# The canonical routing skill, newer than the vendored copy.\n";
    std::fs::create_dir_all(tmp.path().join("skills/workbench-routing")).expect("mkdir");
    std::fs::write(
        tmp.path().join("skills/workbench-routing/SKILL.md"),
        canonical,
    )
    .expect("canonical");

    // The dogfood copy starts as the embedded (markered) version.
    let dogfood = tmp.path().join(".claude/skills/workbench-routing/SKILL.md");
    std::fs::create_dir_all(dogfood.parent().unwrap()).expect("mkdir");
    std::fs::write(&dogfood, owned(".claude/skills/workbench-routing/SKILL.md")).expect("seed");

    let repaired = repair(tmp.path());
    assert!(
        repaired.contains(&".claude/skills/workbench-routing/SKILL.md".to_owned()),
        "the drifted dogfood copy is repaired: {repaired:?}"
    );
    assert_eq!(
        std::fs::read_to_string(&dogfood).expect("read"),
        canonical,
        "byte-equality with the canonical file, no marker"
    );

    // And a copy already at the canonical bytes is left alone.
    let again = repair(tmp.path());
    assert!(
        !again.contains(&".claude/skills/workbench-routing/SKILL.md".to_owned()),
        "a converged copy is not rewritten: {again:?}"
    );
}

fn legacy_workflow(dir: &Path) -> super::super::manifest::ScaffoldManifest {
    let skill = ".claude/skills/next/SKILL.md";
    let old = "# A workflow generated before shared runtime references\n";
    let path = dir.join(skill);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, old).unwrap();
    let mut manifest = super::super::manifest::ScaffoldManifest::new();
    manifest.record(skill, old);
    manifest.save(dir).unwrap();
    manifest
}

#[test]
fn legacy_workflow_upgrade_installs_only_its_missing_runtime_dependency() {
    let tmp = project();
    legacy_workflow(tmp.path());
    let runtime = ".claude/skills/craft/references/runtime.md";
    let repaired = repair(tmp.path());
    assert!(repaired.contains(&runtime.to_owned()));
    assert_eq!(
        std::fs::read_to_string(tmp.path().join(runtime)).unwrap(),
        owned(runtime)
    );
    assert_eq!(
        std::fs::read_to_string(tmp.path().join(".claude/skills/next/SKILL.md")).unwrap(),
        owned(".claude/skills/next/SKILL.md")
    );
    let manifest = super::super::manifest::ScaffoldManifest::load(tmp.path());
    assert!(!manifest.is_customized(runtime, &owned(runtime)));
    let content = std::fs::read_to_string(tmp.path().join(runtime)).unwrap();
    assert!(content.contains("verification: \"focused\""));
    assert!(content.contains("preserve unrelated edits"));
    assert!(content.contains("must not widen focused verification"));
    assert!(!tmp.path().join(".claude/skills/init/SKILL.md").exists());
    assert!(!tmp.path().join(".agents").exists());
    assert!(repair(tmp.path()).is_empty());
}

#[test]
fn legacy_workflow_upgrade_preserves_a_customized_runtime() {
    let tmp = project();
    let mut manifest = legacy_workflow(tmp.path());
    let runtime = ".claude/skills/craft/references/runtime.md";
    let path = tmp.path().join(runtime);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "# Project runtime policy\n").unwrap();
    manifest.record(runtime, "# Previous generated runtime\n");
    manifest.save(tmp.path()).unwrap();
    let _ = repair(tmp.path());
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        "# Project runtime policy\n"
    );
}

#[test]
fn a_deleted_runtime_is_not_recreated_or_required_by_an_upgrade() {
    let tmp = project();
    let mut manifest = legacy_workflow(tmp.path());
    let skill = tmp.path().join(".claude/skills/next/SKILL.md");
    let old = std::fs::read_to_string(&skill).unwrap();
    let runtime = ".claude/skills/craft/references/runtime.md";
    manifest.record(runtime, &owned(runtime));
    manifest.save(tmp.path()).unwrap();
    let _ = repair(tmp.path());
    assert!(!tmp.path().join(runtime).exists());
    assert_eq!(std::fs::read_to_string(skill).unwrap(), old);
}

#[test]
fn malformed_ownership_blocks_the_runtime_migration_before_writes() {
    let tmp = project();
    legacy_workflow(tmp.path());
    let skill = tmp.path().join(".claude/skills/next/SKILL.md");
    let old = std::fs::read_to_string(&skill).unwrap();
    std::fs::write(
        tmp.path().join(".magistr/scaffold-manifest.json"),
        "{broken",
    )
    .unwrap();
    let _ = repair(tmp.path());
    assert!(!tmp.path().join(".claude/skills/craft").exists());
    assert_eq!(std::fs::read_to_string(skill).unwrap(), old);
}

#[cfg(unix)]
#[test]
fn a_runtime_symlink_blocks_the_upgrade_without_writing_its_target() {
    let tmp = project();
    legacy_workflow(tmp.path());
    let skill = tmp.path().join(".claude/skills/next/SKILL.md");
    let old = std::fs::read_to_string(&skill).unwrap();
    let outside = TempDir::new().unwrap();
    let sentinel = outside.path().join("runtime.md");
    std::fs::write(&sentinel, "outside policy").unwrap();
    let references = tmp.path().join(".claude/skills/craft/references");
    std::fs::create_dir_all(references.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(outside.path(), references).unwrap();
    let _ = repair(tmp.path());
    assert_eq!(std::fs::read_to_string(sentinel).unwrap(), "outside policy");
    assert_eq!(std::fs::read_to_string(skill).unwrap(), old);
}

/// A Magistr source checkout (the workflow templates directory beside the
/// project) for a repair: the checkout's own templates are what the owned
/// workflow skills are rendered from.
fn source_checkout(dir: &Path) {
    use super::super::workflow_skills::{Host, WorkflowSkills};
    let workflows = dir.join(super::super::canonical::WORKFLOW_TEMPLATES);
    std::fs::create_dir_all(workflows.join("hosts")).unwrap();
    for (name, _) in WorkflowSkills::workflow_paths(Host::Claude) {
        std::fs::write(
            workflows.join(format!("{name}.md")),
            format!("# {name} ({{{{invoke}}}}{name}) — first edition\n"),
        )
        .unwrap();
    }
    std::fs::write(workflows.join("runtime.md"), "# runtime — first edition\n").unwrap();
    std::fs::write(workflows.join("hosts/claude.md"), "claude adapter\n").unwrap();
    std::fs::write(workflows.join("hosts/codex.md"), "codex adapter\n").unwrap();
    std::fs::write(
        dir.join("crates/magistr-cli/templates/skills/workbench-routing/SKILL.md"),
        "# routing\n",
    )
    .unwrap_or_else(|_| {
        std::fs::create_dir_all(dir.join("crates/magistr-cli/templates/skills/workbench-routing"))
            .unwrap();
        std::fs::write(
            dir.join("crates/magistr-cli/templates/skills/workbench-routing/SKILL.md"),
            "# routing\n",
        )
        .unwrap();
    });
}

/// In Magistr's own checkout an installed binary older than the templates
/// must not downgrade them: repair renders the live templates, follows a
/// later edit without a rebuild, and is idempotent in between. A template
/// that goes missing refuses its skill and never falls back to the
/// embedded copy; ordinary projects (every other test here) keep the
/// embedded upgrades.
#[test]
fn in_a_source_checkout_repair_follows_the_live_templates_and_refuses_missing_ones() {
    let tmp = project();
    let dir = tmp.path();
    source_checkout(dir);
    let skill = ".claude/skills/next/SKILL.md";
    let runtime = ".claude/skills/craft/references/runtime.md";
    // The runtime companion is upgraded only under recorded ownership (an
    // unrecorded one counts as the user's); an earlier setup recorded it.
    let mut manifest = super::super::manifest::ScaffoldManifest::new();
    for path in [skill, runtime] {
        let target = dir.join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, owned(path)).unwrap();
        manifest.record(path, &owned(path));
    }
    manifest.save(dir).unwrap();

    let first = plan(dir);
    let planned = first.get(skill).unwrap();
    assert_eq!(planned.action, FileAction::Overwrite);
    assert_eq!(planned.ownership, Ownership::Canonical);
    assert_eq!(planned.new_content, "# next (/next) — first edition\n");
    let repaired = repair(dir);
    assert!(repaired.contains(&skill.to_owned()), "{repaired:?}");
    assert!(repaired.contains(&runtime.to_owned()), "{repaired:?}");
    assert_eq!(
        std::fs::read_to_string(dir.join(skill)).unwrap(),
        "# next (/next) — first edition\n"
    );
    assert!(
        std::fs::read_to_string(dir.join(runtime))
            .unwrap()
            .ends_with("claude adapter\n")
    );
    assert!(repair(dir).is_empty(), "idempotent once converged");

    // A second edition, and no rebuild in between.
    let templates = dir.join(super::super::canonical::WORKFLOW_TEMPLATES);
    std::fs::write(
        templates.join("next.md"),
        "# next ({{invoke}}next) — second edition\n",
    )
    .unwrap();
    assert_eq!(repair(dir), vec![skill.to_owned()]);
    assert_eq!(
        std::fs::read_to_string(dir.join(skill)).unwrap(),
        "# next (/next) — second edition\n"
    );
    assert!(repair(dir).is_empty());

    // The template disappears: the skill is refused, kept as it is, and the
    // binary's embedded copy is not written in its place.
    std::fs::remove_file(templates.join("next.md")).unwrap();
    let after = plan(dir);
    let refused = after.get(skill).unwrap();
    assert!(
        matches!(
            &refused.action,
            FileAction::Refused(Refusal::CanonicalUnavailable { source, .. })
                if source.ends_with("workflows/next.md")
        ),
        "{:?}",
        refused.action
    );
    assert!(repair(dir).is_empty());
    assert_eq!(
        std::fs::read_to_string(dir.join(skill)).unwrap(),
        "# next (/next) — second edition\n"
    );
    assert_ne!(
        std::fs::read_to_string(dir.join(skill)).unwrap(),
        owned(skill),
        "no stale embedded fallback"
    );
}

/// The repair plan lists a deleted runtime as preserved (not recreated)
/// and the skills that depend on it as refused, so a reviewer sees why an
/// upgrade did not happen.
#[test]
fn the_repair_plan_shows_a_deleted_runtime_and_the_upgrades_it_blocks() {
    let tmp = project();
    let mut manifest = legacy_workflow(tmp.path());
    let runtime = ".claude/skills/craft/references/runtime.md";
    manifest.record(runtime, &owned(runtime));
    manifest.save(tmp.path()).unwrap();

    let listed = plan(tmp.path());
    assert_eq!(listed.operation, AdminOperation::Repair);
    let deleted = listed.get(runtime).unwrap();
    assert_eq!(deleted.action, FileAction::Preserved);
    assert_eq!(
        deleted.reason().as_deref(),
        Some("removed by the user; not recreated")
    );
    let skill = listed.get(".claude/skills/next/SKILL.md").unwrap();
    assert!(
        matches!(
            &skill.action,
            FileAction::Refused(Refusal::MissingDependency { dependency }) if dependency == runtime
        ),
        "{:?}",
        skill.action
    );
    assert!(repair(tmp.path()).is_empty());
}
