use super::*;
use crate::scaffold::vars;

fn test_vars() -> HashMap<String, String> {
    vars([
        ("project_name", "test-project"),
        ("project_description", "A test project"),
        ("language", "rust"),
        ("framework", "none"),
        ("package_manager", "cargo"),
        ("test_framework", "cargo-test"),
        ("linter", "clippy"),
        ("formatter", "rustfmt"),
        ("check_cmd", "cargo check"),
        ("test_cmd", "cargo test"),
        ("lint_cmd", "cargo clippy -- -D warnings"),
        ("format_cmd", "cargo fmt"),
        ("format_extensions", r#""rs""#),
        ("companion_pattern", "tests/{name}.rs"),
        ("test_dir", "tests/"),
        ("state_prefix", "test-project"),
        ("allowed_tools", r#""Bash(cargo:*)""#),
    ])
}

#[test]
fn all_emitters_produce_files() {
    let tv = test_vars();
    for emitter in all_emitters() {
        let files = emitter.files(&tv);
        assert!(
            !files.is_empty(),
            "{} emitter should produce at least one file",
            emitter.name()
        );
    }
}

#[test]
fn codex_emitter_is_registered_without_claiming_shared_files() {
    let emitters = all_emitters();
    let codex = emitters
        .iter()
        .find(|emitter| emitter.id() == "codex")
        .expect("Codex is a scaffold target");
    let files = codex.files(&test_vars());
    assert!(files.iter().any(|file| file.path == ".codex/run"));
    assert!(
        files
            .iter()
            .any(|file| file.path == ".agents/skills/next/SKILL.md")
    );
    for shared in ["AGENTS.md", ".codex/config.toml"] {
        assert!(!files.iter().any(|file| file.path == shared));
        assert!(!codex.removable_paths().contains(&shared));
    }
}

#[test]
fn shared_codex_paths_do_not_select_the_target() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    std::fs::create_dir_all(dir.join(".codex")).unwrap();
    std::fs::create_dir_all(dir.join(".agents/skills/next")).unwrap();
    std::fs::write(dir.join("AGENTS.md"), "User project instructions\n").unwrap();
    std::fs::write(dir.join(".codex/config.toml"), "model = \"user-choice\"\n").unwrap();
    std::fs::write(
        dir.join(".agents/skills/next/SKILL.md"),
        "User next skill\n",
    )
    .unwrap();
    assert!(!agents_present(dir).contains(&"codex".to_owned()));
}

#[test]
fn cursor_emitter_produces_mdc_with_frontmatter() {
    let tv = test_vars();
    let files = CursorEmitter.files(&tv);
    assert_eq!(files.len(), 3);

    for f in &files {
        assert!(f.path.starts_with(".cursor/rules/"));
        assert!(f.path.ends_with(".mdc"));
        assert!(f.content.starts_with("---\n"));
        assert!(f.content.contains("alwaysApply: true"));
    }

    // Conventions should have Rust-specific globs.
    let conv = files
        .iter()
        .find(|f| f.path.contains("conventions"))
        .unwrap();
    assert!(conv.content.contains("**/*.rs"));
    assert!(conv.content.contains("Rust coding conventions"));
}

#[test]
fn cursor_conventions_include_ai_when_present() {
    let mut tv = test_vars();
    tv.insert(
        "ai_conventions".into(),
        "- Use thiserror for errors\n- No unwrap in lib code".into(),
    );
    tv.insert("ai_tips".into(), "- Enable clippy pedantic in CI".into());

    let files = CursorEmitter.files(&tv);
    let conv = files
        .iter()
        .find(|f| f.path.contains("conventions"))
        .unwrap();

    // AI conventions should now appear in Cursor's conventions.mdc
    assert!(conv.content.contains("Use thiserror for errors"));
    assert!(conv.content.contains("No unwrap in lib code"));
    assert!(conv.content.contains("Enable clippy pedantic in CI"));
}

#[test]
fn copilot_cline_windsurf_share_structure() {
    let tv = test_vars();
    let copilot = CopilotEmitter.files(&tv);
    let cline = ClineEmitter.files(&tv);
    let windsurf = WindsurfEmitter.files(&tv);

    // All produce exactly one file with the same content structure.
    assert_eq!(copilot.len(), 1);
    assert_eq!(cline.len(), 1);
    assert_eq!(windsurf.len(), 1);

    for files in [&copilot, &cline, &windsurf] {
        let content = &files[0].content;
        assert!(content.contains("# test-project"));
        assert!(content.contains("**Stack:** rust"));
        assert!(content.contains("## Quick Start"));
        assert!(content.contains("## Workflow"));
        assert!(content.contains("## Conventions"));
    }

    // But different paths.
    assert_eq!(copilot[0].path, ".github/copilot-instructions.md");
    assert_eq!(cline[0].path, ".clinerules");
    assert_eq!(windsurf[0].path, ".windsurfrules");
}

#[test]
fn aider_emitter_produces_yaml() {
    let tv = test_vars();
    let files = AiderEmitter.files(&tv);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, ".aider.conf.yml");
    assert!(files[0].content.contains("lint-cmd:"));
    assert!(files[0].content.contains("test-cmd:"));
    assert!(files[0].content.contains("auto-lint: true"));
}

#[test]
fn instructions_include_ai_conventions() {
    let mut tv = test_vars();
    tv.insert(
        "ai_conventions".into(),
        "- Custom rule one\n- Custom rule two".into(),
    );
    tv.insert("ai_tips".into(), "- Tip for agents".into());
    let content = build_instructions(&tv);
    assert!(content.contains("Custom rule one"));
    assert!(content.contains("Tip for agents"));
}

#[test]
fn instructions_include_framework_when_present() {
    let mut tv = test_vars();
    tv.insert("framework".into(), "next".into());
    let content = build_instructions(&tv);
    assert!(content.contains("**Stack:** rust + next"));
}

#[test]
fn instructions_omit_framework_none() {
    let tv = test_vars();
    let content = build_instructions(&tv);
    assert!(content.contains("**Stack:** rust\n"));
    assert!(!content.contains("+ none"));
}

#[test]
fn language_globs_returns_correct_extensions() {
    assert_eq!(language_globs("rust"), vec!["**/*.rs"]);
    assert!(language_globs("typescript").contains(&"**/*.tsx"));
    assert_eq!(language_globs("python"), vec!["**/*.py"]);
    assert_eq!(language_globs("unknown-lang"), vec!["**/*"]);
}

#[test]
fn remove_emitter_files_cleans_up() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    let tv = test_vars();

    // Create emitter files with evidence of what Magistr wrote.
    let mut manifest = super::super::manifest::ScaffoldManifest::new();
    for emitter in all_emitters() {
        for f in emitter.files(&tv) {
            let full = dir.join(f.path);
            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&full, &f.content).unwrap();
            manifest.record(f.path, &f.content);
        }
    }

    assert!(dir.join(".cursor/rules/workflow.mdc").exists());
    assert!(dir.join(".clinerules").exists());

    let mut removal = super::super::owned_removal::OwnedRemoval::new(dir, &manifest);
    remove_emitter_files(&mut removal).unwrap();
    removal.prune();

    // All emitter files gone.
    assert!(!dir.join(".cursor/rules/workflow.mdc").exists());
    assert!(!dir.join(".cursor/rules/tools.mdc").exists());
    assert!(!dir.join(".clinerules").exists());
    assert!(!dir.join(".windsurfrules").exists());
    assert!(!dir.join(".aider.conf.yml").exists());
    assert!(!dir.join(".github/copilot-instructions.md").exists());

    // Empty dirs pruned.
    assert!(!dir.join(".cursor/rules").exists());
}

#[test]
fn remove_emitter_files_preserves_user_cursor_rules() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();

    // Create magistr's cursor rules + a user-added rule.
    let rules_dir = dir.join(".cursor/rules");
    std::fs::create_dir_all(&rules_dir).unwrap();
    std::fs::write(rules_dir.join("workflow.mdc"), "magistr content").unwrap();
    let mut manifest = super::super::manifest::ScaffoldManifest::new();
    manifest.record(".cursor/rules/workflow.mdc", "magistr content");
    std::fs::write(rules_dir.join("my-custom-rule.mdc"), "user content").unwrap();

    let mut removal = super::super::owned_removal::OwnedRemoval::new(dir, &manifest);
    remove_emitter_files(&mut removal).unwrap();
    removal.prune();

    // Magistr's file gone, user's file preserved, directory stays.
    assert!(!rules_dir.join("workflow.mdc").exists());
    assert!(rules_dir.join("my-custom-rule.mdc").exists());
    assert!(rules_dir.exists());
}

/// A re-init must not silently narrow which agents a project supports:
/// `--yes` takes the default agent set, and without this the emitter
/// files for agents the project already had are removed and never
/// written back. `.clinerules`, `.windsurfrules` and `.aider.conf.yml`
/// all left the real workbench-mcp repo that way.
#[test]
fn the_agents_a_project_already_has_are_reported() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    std::fs::write(dir.join(".clinerules"), "rules").unwrap();
    std::fs::write(dir.join(".windsurfrules"), "rules").unwrap();

    let present = agents_present(dir);
    assert!(present.contains(&"cline".to_string()), "{present:?}");
    assert!(present.contains(&"windsurf".to_string()), "{present:?}");
    assert!(!present.contains(&"cursor".to_string()), "{present:?}");
}

#[test]
fn a_project_with_no_agent_files_reports_none() {
    let tmp = tempfile::TempDir::new().unwrap();
    assert!(agents_present(tmp.path()).is_empty());
}
