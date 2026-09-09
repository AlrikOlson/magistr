use super::*;
use std::collections::HashMap;

pub(super) fn test_vars() -> HashMap<String, String> {
    [
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
        ("reference_repos", ""),
        ("allowed_tools", r#""Bash(cargo:*)""#),
        ("project_type", "cli"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

#[test]
fn roadmap_exports_policy_survives_reinitialization_without_recreating_a_view() {
    let root = tempfile::tempdir().expect("project");
    std::fs::create_dir(root.path().join(".magistr")).expect("config directory");
    let config = r#"{"roadmap_backend":"think-and-ship","roadmap_exports":false}"#;
    std::fs::write(root.path().join(".magistr/project.json"), config).expect("policy");
    let overrides = HashMap::from([("ROADMAP.md".to_owned(), "unrequested view".to_owned())]);
    generate_with_result(root.path(), &test_vars(), &overrides).expect("reinitialize");
    assert!(!root.path().join("ROADMAP.md").exists());
    assert!(!root.path().join("ROADMAP.json").exists());
    assert_eq!(
        std::fs::read_to_string(root.path().join(".magistr/project.json")).expect("config"),
        config
    );
}

fn assert_preview_outcomes(
    dir: &Path,
    vars: &HashMap<String, String>,
    overrides: &HashMap<String, String>,
    expected: &[(&str, super::super::FileAction)],
) {
    use super::super::FileAction;
    let preview = super::super::preview(dir, vars, overrides).unwrap();
    for (path, action) in expected {
        let entry = preview.files.iter().find(|f| f.path == *path).unwrap();
        assert_eq!(&entry.action, action, "{path}");
        if *action == FileAction::Preserved {
            assert_eq!(
                entry.new_content,
                std::fs::read_to_string(dir.join(path)).unwrap()
            );
            assert!(entry.diff.is_none(), "{path}");
        }
    }
    let result = generate_with_result(dir, vars, overrides).unwrap();
    for (path, action) in expected {
        let paths = match action {
            FileAction::Create => &result.created,
            FileAction::Overwrite => &result.updated,
            FileAction::Unchanged => &result.unchanged,
            FileAction::Preserved => &result.preserved,
        };
        assert!(paths.iter().any(|p| p == path), "{path}: {result:?}");
        let entry = preview.files.iter().find(|f| f.path == *path).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join(path)).unwrap(),
            entry.new_content,
            "{path}"
        );
    }
}

#[test]
fn preview_preserves_customized_and_unowned_regular_files() {
    use super::super::FileAction::Preserved;
    let tmp = tempfile::tempdir().unwrap();
    let vars = test_vars();
    generate_with_result(tmp.path(), &vars, &HashMap::new()).unwrap();
    for path in [".claude/settings.json", ".claude/rules/conventions.md"] {
        std::fs::write(tmp.path().join(path), "user's own content\n").unwrap();
    }
    assert_preview_outcomes(
        tmp.path(),
        &vars,
        &HashMap::new(),
        &[
            (".claude/settings.json", Preserved),
            (".claude/rules/conventions.md", Preserved),
        ],
    );

    // An existing file with no manifest entry belongs to its author too.
    let unowned = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(unowned.path().join(".claude")).unwrap();
    std::fs::write(
        unowned.path().join(".claude/settings.json"),
        "{\"user\": true}\n",
    )
    .unwrap();
    assert_preview_outcomes(
        unowned.path(),
        &vars,
        &HashMap::new(),
        &[(".claude/settings.json", Preserved)],
    );
}

#[test]
fn custom_magistr_gitignore_survives_preview_and_reinit() {
    use super::super::FileAction::Preserved;
    let vars = test_vars();
    let relative = ".magistr/.gitignore";
    let custom = "*\n!.gitignore\n!project.json\n!lint-structure-allowlist.txt\n";

    for recorded in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        if recorded {
            generate_with_result(tmp.path(), &vars, &HashMap::new()).unwrap();
        } else {
            std::fs::create_dir(tmp.path().join(".magistr")).unwrap();
        }
        std::fs::write(tmp.path().join(relative), custom).unwrap();

        for _ in 0..2 {
            assert_preview_outcomes(tmp.path(), &vars, &HashMap::new(), &[(relative, Preserved)]);
            assert_eq!(
                std::fs::read_to_string(tmp.path().join(relative)).unwrap(),
                custom
            );
        }
    }
}

#[test]
fn preview_matches_regular_file_creation_noop_and_template_upgrade() {
    use super::super::FileAction::{Create, Overwrite, Unchanged};
    let tmp = tempfile::tempdir().unwrap();
    let vars = test_vars();
    let paths = [
        ".claude/settings.json",
        ".claude/rules/conventions.md",
        ".claude/rules/tools.md",
        ".magistr/.gitignore",
    ];
    let expected = |action: super::super::FileAction| {
        paths
            .iter()
            .map(|path| (*path, action.clone()))
            .collect::<Vec<_>>()
    };
    assert_preview_outcomes(tmp.path(), &vars, &HashMap::new(), &expected(Create));
    assert_preview_outcomes(tmp.path(), &vars, &HashMap::new(), &expected(Unchanged));

    // A newly rendered template, including an AI override, can replace an
    // unmodified generated file. A subsequent user edit must still win.
    let overrides = HashMap::from([
        (paths[0].to_owned(), "{\"new_template\": true}\n".to_owned()),
        (paths[1].to_owned(), "# New conventions\n".to_owned()),
        (paths[2].to_owned(), "# New tool guide\n".to_owned()),
        (
            paths[3].to_owned(),
            "*\n!.gitignore\n!project.json\n".to_owned(),
        ),
    ]);
    assert_preview_outcomes(tmp.path(), &vars, &overrides, &expected(Overwrite));
    std::fs::write(tmp.path().join(paths[0]), "{\"user\": true}\n").unwrap();
    assert_preview_outcomes(
        tmp.path(),
        &vars,
        &HashMap::new(),
        &[
            (paths[0], super::super::FileAction::Preserved),
            (paths[1], Overwrite),
            (paths[2], Overwrite),
            (paths[3], Overwrite),
        ],
    );
}

#[test]
fn fresh_generate_creates_all_files() {
    let tmp = tempfile::TempDir::new().unwrap();
    let result = generate_with_result(tmp.path(), &test_vars(), &HashMap::new()).unwrap();

    assert!(!result.created.is_empty(), "should create files");
    assert!(
        result.updated.is_empty() || result.updated == vec![".mcp.json", ".gitignore"],
        "only merge files should be 'updated' on fresh init"
    );
    assert!(
        result.preserved.is_empty(),
        "nothing to preserve on fresh init"
    );
}

#[test]
fn reinit_skips_unchanged_files() {
    let tmp = tempfile::TempDir::new().unwrap();
    let vars = test_vars();
    let no_overrides = HashMap::new();

    // First init
    generate_with_result(tmp.path(), &vars, &no_overrides).unwrap();

    // Second init (reinit) — same vars, everything should be unchanged
    let result = generate_with_result(tmp.path(), &vars, &no_overrides).unwrap();

    assert!(result.created.is_empty(), "no new files on reinit");
    assert!(
        !result.unchanged.is_empty(),
        "most files should be unchanged: {:?}",
        result.unchanged
    );
    // CLAUDE.md is NeverOverwrite → preserved
    assert!(
        result.preserved.contains(&"CLAUDE.md".to_string()),
        "CLAUDE.md should be preserved"
    );
}

#[test]
fn reinit_preserves_user_customized_settings() {
    let tmp = tempfile::TempDir::new().unwrap();
    let vars = test_vars();
    let no_overrides = HashMap::new();

    // First init — creates manifest
    generate_with_result(tmp.path(), &vars, &no_overrides).unwrap();

    // User customizes settings.local.json
    let settings_path = tmp.path().join(".claude/settings.local.json");
    std::fs::write(&settings_path, r#"{"my": "custom settings"}"#).unwrap();

    // Reinit — should preserve the user's customized file
    let result = generate_with_result(tmp.path(), &vars, &no_overrides).unwrap();

    assert!(
        result
            .preserved
            .contains(&".claude/settings.local.json".to_string()),
        "customized settings.local.json should be preserved, got: {:?}",
        result
    );

    // Verify the content is still the user's custom version
    let content = std::fs::read_to_string(&settings_path).unwrap();
    assert_eq!(content, r#"{"my": "custom settings"}"#);
}

#[test]
fn reinit_preserves_user_customized_conventions() {
    let tmp = tempfile::TempDir::new().unwrap();
    let vars = test_vars();
    let no_overrides = HashMap::new();

    // First init
    generate_with_result(tmp.path(), &vars, &no_overrides).unwrap();

    // User customizes conventions.md
    let conventions_path = tmp.path().join(".claude/rules/conventions.md");
    std::fs::write(
        &conventions_path,
        "# My custom conventions\n\nDo things my way.",
    )
    .unwrap();

    // Reinit — should preserve customizations
    let result = generate_with_result(tmp.path(), &vars, &no_overrides).unwrap();

    assert!(
        result
            .preserved
            .contains(&".claude/rules/conventions.md".to_string()),
        "customized conventions.md should be preserved"
    );

    let content = std::fs::read_to_string(&conventions_path).unwrap();
    assert!(content.contains("My custom conventions"));
}

#[test]
fn reinit_updates_overwrite_policy_files_when_template_changes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let vars = test_vars();
    let no_overrides = HashMap::new();

    // First init
    generate_with_result(tmp.path(), &vars, &no_overrides).unwrap();

    // Simulate a template change by modifying an Overwrite-policy file
    let tools_path = tmp.path().join(".claude/rules/tools.md");
    std::fs::write(&tools_path, "old version of tool guide").unwrap();

    // Reinit — should overwrite tools.md even though user "changed" it
    let result = generate_with_result(tmp.path(), &vars, &no_overrides).unwrap();

    assert!(
        result
            .updated
            .contains(&".claude/rules/tools.md".to_string()),
        "Overwrite-policy file should be updated when content differs"
    );
}

#[test]
fn reinit_updates_uncustomized_preserve_files_when_template_changes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut vars = test_vars();
    let no_overrides = HashMap::new();

    // First init with rust conventions
    generate_with_result(tmp.path(), &vars, &no_overrides).unwrap();

    // Change language → different conventions template
    vars.insert("language".to_string(), "python".to_string());

    // Reinit — conventions.md was not customized by the user (matches manifest),
    // so it should be updated to the new language's conventions
    let result = generate_with_result(tmp.path(), &vars, &no_overrides).unwrap();

    assert!(
        result
            .updated
            .contains(&".claude/rules/conventions.md".to_string()),
        "Uncustomized PreserveCustomized file should update when template changes"
    );

    let content = std::fs::read_to_string(tmp.path().join(".claude/rules/conventions.md")).unwrap();
    assert!(
        content.contains("Python") || content.contains("python"),
        "conventions.md should now contain Python conventions"
    );
}

#[test]
fn manifest_saved_after_generate() {
    let tmp = tempfile::TempDir::new().unwrap();
    let vars = test_vars();
    generate_with_result(tmp.path(), &vars, &HashMap::new()).unwrap();

    let manifest_path = tmp.path().join(".magistr/scaffold-manifest.json");
    assert!(
        manifest_path.is_file(),
        "manifest should be saved after generate"
    );

    let content = std::fs::read_to_string(&manifest_path).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&content).unwrap();
    let files = manifest["files"].as_object().unwrap();
    assert!(!files.is_empty(), "manifest should contain file hashes");
}

#[test]
fn generate_result_written_returns_created_plus_updated() {
    let result = GenerateResult {
        created: vec!["a.md".to_string()],
        updated: vec!["b.md".to_string()],
        unchanged: vec!["c.md".to_string()],
        preserved: vec!["d.md".to_string()],
    };

    let written = result.written();
    assert_eq!(written, vec!["a.md", "b.md"]);
}

#[test]
fn reinit_never_overwrites_claude_md() {
    let tmp = tempfile::TempDir::new().unwrap();
    let vars = test_vars();
    let no_overrides = HashMap::new();

    // First init
    generate_with_result(tmp.path(), &vars, &no_overrides).unwrap();

    // Even if we change the template vars, CLAUDE.md should never be overwritten
    let claude_path = tmp.path().join("CLAUDE.md");
    let original = std::fs::read_to_string(&claude_path).unwrap();
    std::fs::write(&claude_path, "# User's custom CLAUDE.md").unwrap();

    let result = generate_with_result(tmp.path(), &vars, &no_overrides).unwrap();

    assert!(result.preserved.contains(&"CLAUDE.md".to_string()));
    let content = std::fs::read_to_string(&claude_path).unwrap();
    // The user's text is kept verbatim; only the marked think-and-ship
    // section is (re)appended after it.
    assert!(content.starts_with("# User's custom CLAUDE.md\n\n<!-- think-and-ship -->"));
    assert_ne!(content, original);
}
