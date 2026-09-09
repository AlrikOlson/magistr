use super::*;
use std::collections::BTreeSet;

fn skill_names(files: &[ScaffoldFile]) -> BTreeSet<&str> {
    files
        .iter()
        .filter(|file| file.path.ends_with("/SKILL.md"))
        .map(|file| {
            let metadata = file.content.strip_prefix("---\n").expect("frontmatter");
            let metadata = metadata.split_once("\n---\n").expect("frontmatter end").0;
            let name = metadata
                .lines()
                .find_map(|line| line.strip_prefix("name: "))
                .expect("skill name");
            assert!(
                metadata
                    .lines()
                    .any(|line| line.starts_with("description: "))
            );
            assert_eq!(file.path.rsplit('/').nth(1), Some(name));
            name
        })
        .collect()
}

#[test]
fn each_host_discovers_the_same_nine_skills() {
    let codex = WorkflowSkills::codex_files();
    let claude = WorkflowSkills::claude_files();
    let expected = BTreeSet::from([
        "craft",
        "init",
        "next",
        "next-loop",
        "roadmap",
        "roadmap-refresh",
        "signals",
        "validate",
        "workbench-routing",
    ]);
    assert_eq!(skill_names(&codex), expected);
    assert_eq!(skill_names(&claude), expected);
    assert!(
        codex
            .iter()
            .all(|file| file.path.starts_with(".agents/skills/"))
    );
    assert!(
        claude
            .iter()
            .all(|file| file.path.starts_with(".claude/skills/"))
    );
    assert_eq!(codex.len(), 10, "nine skills and their shared runtime");
    assert_eq!(claude.len(), 10);
}

#[test]
fn customized_codex_skills_and_their_reference_survive_reinit() {
    let files = WorkflowSkills::codex_files();
    assert!(!files.is_empty());
    assert!(
        files
            .iter()
            .all(|file| matches!(file.policy, UpdatePolicy::PreserveCustomized))
    );
    assert!(files.iter().all(|file| !file.executable));
}

#[test]
fn workflow_semantics_are_identical_across_hosts() {
    let codex = WorkflowSkills::codex_files();
    let claude = WorkflowSkills::claude_files();
    let names = skill_names(&claude);
    for (codex, claude) in codex.iter().zip(&claude) {
        assert_eq!(
            codex.path.strip_prefix(".agents/"),
            claude.path.strip_prefix(".claude/")
        );
        if codex.path.ends_with("runtime.md") {
            let codex_shared = codex
                .content
                .split_once("<!-- Host adapter -->")
                .expect("adapter")
                .0;
            let claude_shared = claude
                .content
                .split_once("<!-- Host adapter -->")
                .expect("adapter")
                .0;
            assert_eq!(codex_shared, claude_shared);
            assert_ne!(codex.content, claude.content, "host mechanics differ");
        } else if codex.path.ends_with("workbench-routing/SKILL.md") {
            assert_eq!(codex.content, claude.content);
            assert!(
                codex
                    .content
                    .starts_with(super::super::STATIC_SKILL_WORKBENCH_ROUTING.trim_end())
            );
        } else {
            let mut normalized = codex.content.clone();
            for name in &names {
                normalized = normalized.replace(&format!("${name}"), &format!("/{name}"));
            }
            assert_eq!(normalized, claude.content, "{}", codex.path);
        }
        assert!(
            !codex.content.contains("{{"),
            "unexpanded token in {}",
            codex.path
        );
        assert!(
            !claude.content.contains("{{"),
            "unexpanded token in {}",
            claude.path
        );
    }
}

#[test]
fn every_generated_relative_workflow_link_resolves() {
    let temporary = tempfile::tempdir().expect("tempdir");
    for files in [
        WorkflowSkills::codex_files(),
        WorkflowSkills::claude_files(),
    ] {
        for file in &files {
            let path = temporary.path().join(file.path);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(path, &file.content).expect("write");
        }
        for file in &files {
            for after in file.content.split("](").skip(1) {
                let link = after.split(')').next().expect("link");
                if link.ends_with(".md") && !link.contains("://") {
                    let parent = temporary
                        .path()
                        .join(file.path)
                        .parent()
                        .expect("parent")
                        .to_path_buf();
                    assert!(
                        parent.join(link).is_file(),
                        "{} refers to missing {link}",
                        file.path
                    );
                }
            }
        }
    }
}
