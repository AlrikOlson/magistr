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
    let config = serde_json::json!({
    "project": {"name": "settings-test", "type": "cli"},
    "stack": {"language": "unknown", "framework": "none", "package_manager": "unknown", "test_framework": "none", "linter": "none", "formatter": "none"},
    "quality_gate": {"check": "true", "test": "true", "lint": "true"},
    "format": {"command": "", "extensions": []},
    "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
    "reference_repos": [], "state_prefix": "settings-test", "roadmap_exports": false
}).to_string();
    std::fs::write(root.path().join(".magistr/project.json"), &config).expect("policy");
    let overrides = HashMap::from([("ROADMAP.md".to_owned(), "unrequested view".to_owned())]);
    generate_with_result(
        root.path(),
        &test_vars(),
        &overrides,
        &crate::scaffold::SetupSeeds::default(),
    )
    .expect("reinitialize");
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
    let preview = super::super::preview(
        dir,
        vars,
        overrides,
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();
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
    let result = generate_with_result(
        dir,
        vars,
        overrides,
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();
    for (path, action) in expected {
        let paths = match action {
            FileAction::Create => &result.created,
            FileAction::Overwrite => &result.updated,
            FileAction::Unchanged => &result.unchanged,
            FileAction::Preserved => &result.preserved,
            FileAction::Remove => &result.removed,
            FileAction::Refused(_) => &result.refused,
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
    generate_with_result(
        tmp.path(),
        &vars,
        &HashMap::new(),
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();
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
            generate_with_result(
                tmp.path(),
                &vars,
                &HashMap::new(),
                &crate::scaffold::SetupSeeds::default(),
            )
            .unwrap();
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
    let result = generate_with_result(
        tmp.path(),
        &test_vars(),
        &HashMap::new(),
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();

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
    generate_with_result(
        tmp.path(),
        &vars,
        &no_overrides,
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();

    // Second init (reinit) — same vars, everything should be unchanged
    let result = generate_with_result(
        tmp.path(),
        &vars,
        &no_overrides,
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();

    assert!(result.created.is_empty(), "no new files on reinit");
    assert!(
        !result.unchanged.is_empty(),
        "most files should be unchanged: {:?}",
        result.unchanged
    );
    // CLAUDE.md is the user's file; its think-and-ship section is already
    // current, so the plan lists it as unchanged rather than rewriting it.
    assert!(
        result.unchanged.contains(&"CLAUDE.md".to_string()),
        "CLAUDE.md should be unchanged: {result:?}"
    );
    assert!(
        result
            .preserved
            .contains(&".magistr/project.json".to_string()),
        "project.json is never overwritten"
    );
}

#[test]
fn reinit_preserves_user_customized_settings() {
    let tmp = tempfile::TempDir::new().unwrap();
    let vars = test_vars();
    let no_overrides = HashMap::new();

    // First init — creates manifest
    generate_with_result(
        tmp.path(),
        &vars,
        &no_overrides,
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();

    // User customizes settings.local.json
    let settings_path = tmp.path().join(".claude/settings.local.json");
    std::fs::write(&settings_path, r#"{"my": "custom settings"}"#).unwrap();

    // Reinit — should preserve the user's customized file
    let result = generate_with_result(
        tmp.path(),
        &vars,
        &no_overrides,
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();

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
    generate_with_result(
        tmp.path(),
        &vars,
        &no_overrides,
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();

    // User customizes conventions.md
    let conventions_path = tmp.path().join(".claude/rules/conventions.md");
    std::fs::write(
        &conventions_path,
        "# My custom conventions\n\nDo things my way.",
    )
    .unwrap();

    // Reinit — should preserve customizations
    let result = generate_with_result(
        tmp.path(),
        &vars,
        &no_overrides,
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();

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
    generate_with_result(
        tmp.path(),
        &vars,
        &no_overrides,
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();

    // Simulate a template change by modifying an Overwrite-policy file
    let tools_path = tmp.path().join(".claude/rules/tools.md");
    std::fs::write(&tools_path, "old version of tool guide").unwrap();

    // Reinit — should overwrite tools.md even though user "changed" it
    let result = generate_with_result(
        tmp.path(),
        &vars,
        &no_overrides,
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();

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
    generate_with_result(
        tmp.path(),
        &vars,
        &no_overrides,
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();

    // Change language → different conventions template
    vars.insert("language".to_string(), "python".to_string());

    // Reinit — conventions.md was not customized by the user (matches manifest),
    // so it should be updated to the new language's conventions
    let result = generate_with_result(
        tmp.path(),
        &vars,
        &no_overrides,
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();

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
    generate_with_result(
        tmp.path(),
        &vars,
        &HashMap::new(),
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();

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
        removed: Vec::new(),
        refused: Vec::new(),
        failed: Vec::new(),
        manifest: ManifestOutcome::Saved,
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
    generate_with_result(
        tmp.path(),
        &vars,
        &no_overrides,
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();

    // Even if we change the template vars, CLAUDE.md should never be overwritten
    let claude_path = tmp.path().join("CLAUDE.md");
    let original = std::fs::read_to_string(&claude_path).unwrap();
    std::fs::write(&claude_path, "# User's custom CLAUDE.md").unwrap();

    let result = generate_with_result(
        tmp.path(),
        &vars,
        &no_overrides,
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();

    // The plan says what happens: the user's document is merged with the
    // section, so it is reported as updated, never as preserved-and-then-
    // rewritten behind the preview's back.
    assert!(result.updated.contains(&"CLAUDE.md".to_string()));
    let content = std::fs::read_to_string(&claude_path).unwrap();
    // The user's text is kept verbatim; only the marked think-and-ship
    // section is (re)appended after it.
    assert!(content.starts_with("# User's custom CLAUDE.md\n\n<!-- think-and-ship -->"));
    assert_ne!(content, original);
}

/// The plan is what generation executes: every applied change lands with
/// the planned bytes, every removal is gone, and a second plan over the
/// result has nothing left to apply. Exercised on a project that carries
/// everything setup merges or retires: a retired rule file, a retired MCP
/// server, a user `.gitignore`, a `CLAUDE.md` without its section and a
/// `workbench.toml` without a confirm mode.
#[test]
fn the_setup_plan_is_what_generation_writes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    let vars = test_vars();
    std::fs::create_dir_all(dir.join(".claude/rules")).unwrap();
    std::fs::write(dir.join(".claude/rules/ministr-scope.md"), "retired\n").unwrap();
    std::fs::write(
        dir.join(".mcp.json"),
        r#"{"mcpServers":{"think-and-ship":{"command":"think-and-ship"},"mine":{"command":"mine"}}}"#,
    )
    .unwrap();
    std::fs::write(dir.join(".gitignore"), "mine/\n").unwrap();
    std::fs::write(dir.join("CLAUDE.md"), "# Ours\n").unwrap();
    std::fs::write(
        dir.join("workbench.toml"),
        "[commands]\n\"test\" = \"cargo test\"\n",
    )
    .unwrap();

    let plan = super::super::preview(
        dir,
        &vars,
        &HashMap::new(),
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();
    assert_eq!(plan.operation, super::super::AdminOperation::Setup);
    assert_eq!(plan.refusals(), 0, "{:?}", plan.refused());
    let rule = plan.get(".claude/rules/ministr-scope.md").unwrap();
    assert_eq!(rule.action, super::super::FileAction::Remove);
    let mcp = plan.get(".mcp.json").unwrap();
    assert_eq!(mcp.action, super::super::FileAction::Overwrite);
    assert_eq!(mcp.ownership, super::super::Ownership::Shared);
    assert!(
        !mcp.new_content.contains("think-and-ship"),
        "retired server stripped in the plan"
    );
    assert!(mcp.new_content.contains("\"mine\""), "user entry kept");
    let gitignore = plan.get(".gitignore").unwrap();
    assert_eq!(gitignore.action, super::super::FileAction::Overwrite);
    assert!(gitignore.new_content.starts_with("mine/\n"));
    let claude = plan.get("CLAUDE.md").unwrap();
    assert_eq!(claude.action, super::super::FileAction::Overwrite);
    assert!(
        claude
            .new_content
            .starts_with("# Ours\n\n<!-- think-and-ship -->")
    );
    let workbench = plan.get("workbench.toml").unwrap();
    assert_eq!(workbench.action, super::super::FileAction::Overwrite);
    assert!(workbench.new_content.contains("confirm_mode = \"token\""));
    assert!(
        workbench
            .new_content
            .starts_with("[commands]\n\"test\" = \"cargo test\"\n")
    );
    for f in &plan.files {
        assert_eq!(
            f.expected(),
            super::super::Expected::of(f.existing_content.as_deref())
        );
    }

    let result = generate_with_result(
        dir,
        &vars,
        &HashMap::new(),
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();
    assert!(result.refused.is_empty());
    assert_eq!(result.removed, vec![".claude/rules/ministr-scope.md"]);
    assert!(!dir.join(".claude/rules/ministr-scope.md").exists());
    for f in plan.applied() {
        if f.action == super::super::FileAction::Remove {
            continue;
        }
        assert_eq!(
            std::fs::read_to_string(dir.join(&f.path)).unwrap(),
            f.new_content,
            "{} lands exactly as planned",
            f.path
        );
        let bucket = if f.existing_content.is_some() {
            &result.updated
        } else {
            &result.created
        };
        assert!(bucket.contains(&f.path), "{}: {result:?}", f.path);
    }

    let again = super::super::preview(
        dir,
        &vars,
        &HashMap::new(),
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();
    assert_eq!(
        again.applies(),
        0,
        "{:?}",
        again.applied().collect::<Vec<_>>()
    );
    assert_eq!(again.refusals(), 0);
    assert_eq!(
        again.get("CLAUDE.md").unwrap().action,
        super::super::FileAction::Unchanged
    );
}

/// A directory (or anything that is not a regular file) at a scaffold path
/// is refused in the plan and left alone by generation; the rest of the
/// scaffold still lands and the result says what was refused.
#[test]
fn a_directory_at_a_scaffold_path_is_refused_not_overwritten() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    std::fs::create_dir_all(dir.join(".claude/rules/tools.md")).unwrap();
    let plan = super::super::preview(
        dir,
        &test_vars(),
        &HashMap::new(),
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();
    let tools = plan.get(".claude/rules/tools.md").unwrap();
    assert_eq!(
        tools.action,
        super::super::FileAction::Refused(super::super::Refusal::NotRegularFile)
    );
    assert_eq!(plan.refusals(), 1);

    let result = generate_with_result(
        dir,
        &test_vars(),
        &HashMap::new(),
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();
    assert_eq!(result.refused, vec![".claude/rules/tools.md"]);
    assert!(dir.join(".claude/rules/tools.md").is_dir());
    assert!(dir.join(".claude/rules/workflow.md").is_file());
    assert!(dir.join(".magistr/project.json").is_file());
}

/// Fault injection on generation: one upgrade cannot be written (its
/// temporary file name is taken), so the target keeps its bytes, the other
/// changes still land, the result names the failure, and `generate` refuses
/// to call that a success while saying what was applied.
#[test]
fn a_setup_with_one_failed_write_reports_the_partial_outcome() {
    let tmp = tempfile::TempDir::new().unwrap();
    let vars = test_vars();
    generate_with_result(
        tmp.path(),
        &vars,
        &HashMap::new(),
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();
    let settings = ".claude/settings.json";
    let before = std::fs::read_to_string(tmp.path().join(settings)).unwrap();
    let overrides = HashMap::from([
        (settings.to_owned(), "{\"upgraded\": true}\n".to_owned()),
        (
            ".claude/rules/tools.md".to_owned(),
            "# upgraded tool guide\n".to_owned(),
        ),
    ]);
    std::fs::write(
        super::super::admin_service::temp_path(&tmp.path().join(settings)),
        "occupied",
    )
    .unwrap();

    let error = generate(
        tmp.path(),
        &vars,
        &overrides,
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("setup did not complete"), "{error}");
    assert!(error.contains(".claude/settings.json: failed"), "{error}");
    assert!(error.contains("applied before that: "), "{error}");
    assert!(error.contains(".claude/rules/tools.md"), "{error}");
    assert_eq!(
        std::fs::read_to_string(tmp.path().join(settings)).unwrap(),
        before
    );
    assert_eq!(
        std::fs::read_to_string(tmp.path().join(".claude/rules/tools.md")).unwrap(),
        "# upgraded tool guide\n"
    );

    // The same run, seen through the detailed result.
    let result = generate_with_result(
        tmp.path(),
        &vars,
        &overrides,
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();
    assert_eq!(result.failed.len(), 1, "{:?}", result.failed);
    assert_eq!(result.failed[0].0, settings);
    assert!(
        result.failed[0].1.starts_with("write failed"),
        "{}",
        result.failed[0].1
    );
    assert!(
        result
            .unchanged
            .contains(&".claude/rules/tools.md".to_owned())
    );
    assert_eq!(result.manifest, ManifestOutcome::Saved);
}

#[test]
fn current_settings_setup_refuses_invalid_declarations_before_any_write() {
    for invalid in [
        serde_json::json!(null),
        serde_json::json!("invalid"),
        serde_json::json!(42),
    ] {
        let root = tempfile::tempdir().unwrap();
        let mut config = serde_json::json!({
            "project": {"name": "settings-test", "type": "cli"},
            "stack": {"language": "unknown", "framework": "none", "package_manager": "unknown", "test_framework": "none", "linter": "none", "formatter": "none"},
            "quality_gate": {"check": "true", "test": "true", "lint": "true"},
            "format": {"command": "", "extensions": []},
            "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
            "reference_repos": [], "state_prefix": "settings-test", "roadmap_exports": false
        });
        config["roadmap_backend"] = invalid;
        std::fs::create_dir(root.path().join(".magistr")).unwrap();
        let path = root.path().join(".magistr/project.json");
        let original = config.to_string();
        std::fs::write(&path, &original).unwrap();
        let result = generate_with_result(
            root.path(),
            &test_vars(),
            &HashMap::new(),
            &crate::scaffold::SetupSeeds::default(),
        );
        assert!(result.is_err(), "invalid settings must refuse setup");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        assert!(!root.path().join(".claude").exists());
        assert!(!root.path().join(".mcp.json").exists());
        assert!(!root.path().join("workbench.toml").exists());
    }
}

#[test]
fn current_yaml_setup_preserves_settings_without_reconstructing_a_roadmap() {
    let root = tempfile::tempdir().unwrap();
    let mut config = serde_json::json!({
        "project": {"name": "settings-test", "type": "cli"},
        "stack": {"language": "unknown", "framework": "none", "package_manager": "unknown", "test_framework": "none", "linter": "none", "formatter": "none"},
        "quality_gate": {"check": "true", "test": "true", "lint": "true"},
        "format": {"command": "", "extensions": []},
        "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
        "reference_repos": [], "state_prefix": "settings-test", "roadmap_exports": false
    });
    config["roadmap_backend"] = "yaml".into();
    std::fs::create_dir(root.path().join(".magistr")).unwrap();
    std::fs::write(
        root.path().join(".magistr/project.json"),
        config.to_string(),
    )
    .unwrap();
    let overrides = HashMap::from([(
        ".magistr-roadmap.yml".to_owned(),
        "title: Current Project\n".to_owned(),
    )]);
    let result = generate_with_result(
        root.path(),
        &test_vars(),
        &overrides,
        &crate::scaffold::SetupSeeds::default(),
    )
    .unwrap();
    assert!(!root.path().join(".magistr-roadmap.yml").exists());
    assert!(!result.created.contains(&".magistr-roadmap.yml".to_owned()));
    assert!(!root.path().join("ROADMAP.md").exists());
    assert!(!root.path().join(".magistr/roadmap.yaml").exists());
}
