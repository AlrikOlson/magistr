use super::*;

#[test]
fn substitutes_single_placeholder() {
    let vars = vars([("name", "magistr")]);
    assert_eq!(substitute("Hello {{name}}!", &vars), "Hello magistr!");
}

#[test]
fn substitutes_multiple_placeholders() {
    let vars = vars([("lang", "rust"), ("fw", "none")]);
    let result = substitute("{{lang}} + {{fw}}", &vars);
    assert_eq!(result, "rust + none");
}

#[test]
fn substitutes_repeated_placeholder() {
    let vars = vars([("x", "yes")]);
    assert_eq!(substitute("{{x}} and {{x}}", &vars), "yes and yes");
}

#[test]
fn leaves_unknown_placeholders() {
    let vars = vars([("a", "1")]);
    assert_eq!(substitute("{{a}} {{b}}", &vars), "1 {{b}}");
}

#[test]
fn handles_empty_template() {
    let vars = vars([("x", "y")]);
    assert_eq!(substitute("", &vars), "");
}

#[test]
fn handles_empty_value() {
    let vars = vars([("x", "")]);
    assert_eq!(substitute("before{{x}}after", &vars), "beforeafter");
}

#[test]
fn handles_json_template() {
    let vars = vars([
        ("project_name", "my-app"),
        ("language", "typescript"),
        ("check_cmd", "npx tsc --noEmit"),
    ]);
    let template =
        r#"{"name": "{{project_name}}", "lang": "{{language}}", "check": "{{check_cmd}}"}"#;
    let result = substitute(template, &vars);
    assert_eq!(
        result,
        r#"{"name": "my-app", "lang": "typescript", "check": "npx tsc --noEmit"}"#
    );
}

#[test]
fn generate_creates_all_scaffold_files() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();

    let template_vars = vars([
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
    ]);

    let written = generate(dir, &template_vars, &HashMap::new()).unwrap();

    // All scaffold files + .mcp.json + .gitignore.
    assert!(
        written.len() >= 20,
        "Expected at least 20 files, got {}",
        written.len()
    );

    // Verify key files exist.
    assert!(dir.join(".magistr/project.json").is_file());
    assert!(dir.join("CLAUDE.md").is_file());
    assert!(dir.join(".claude/settings.json").is_file());
    assert!(dir.join(".claude/settings.local.json").is_file());
    assert!(dir.join(".claude/rules/workflow.md").is_file());
    assert!(dir.join(".claude/rules/tools.md").is_file());
    assert!(dir.join(".claude/rules/conventions.md").is_file());
    // The guard hook behind the Stop / PreToolUse entries in settings.json.
    let hook = dir.join(".claude/hooks/magistr-guard.sh");
    assert!(hook.is_file(), "magistr-guard.sh is scaffolded");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&hook).unwrap().permissions().mode();
        assert!(mode & 0o111 != 0, "hook must be executable, mode {mode:o}");
    }
    assert!(dir.join(".claude/skills/init/SKILL.md").is_file());
    assert!(dir.join(".claude/skills/next/SKILL.md").is_file());
    assert!(dir.join(".claude/skills/next-loop/SKILL.md").is_file());
    assert!(dir.join(".claude/skills/roadmap/SKILL.md").is_file());
    assert!(
        dir.join(".claude/skills/roadmap-refresh/SKILL.md")
            .is_file()
    );
    assert!(dir.join(".claude/skills/signals/SKILL.md").is_file());
    assert!(dir.join(".claude/skills/craft/SKILL.md").is_file());
    assert!(dir.join(".claude/skills/validate/SKILL.md").is_file());
    assert!(dir.join(".mcp.json").is_file());

    // Agent-agnostic emitter files.
    assert!(dir.join(".cursor/rules/workflow.mdc").is_file());
    assert!(dir.join(".cursor/rules/conventions.mdc").is_file());
    assert!(dir.join(".cursor/rules/tools.mdc").is_file());
    assert!(dir.join(".github/copilot-instructions.md").is_file());
    assert!(dir.join(".clinerules").is_file());
    assert!(dir.join(".windsurfrules").is_file());
    assert!(dir.join(".aider.conf.yml").is_file());
}

#[test]
fn generate_substitutes_template_values() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();

    let template_vars = vars([
        ("project_name", "my-app"),
        ("project_description", "My cool app"),
        ("language", "typescript"),
        ("framework", "next"),
        ("package_manager", "pnpm"),
        ("test_framework", "vitest"),
        ("linter", "eslint"),
        ("formatter", "prettier"),
        ("check_cmd", "npx tsc --noEmit"),
        ("test_cmd", "npx vitest run"),
        ("lint_cmd", "npx eslint ."),
        ("format_cmd", "npx prettier --write ."),
        ("format_extensions", r#""ts", "tsx""#),
        ("companion_pattern", "{name}.test.ts"),
        ("test_dir", "__tests__/"),
        ("state_prefix", "my-app"),
        ("allowed_tools", r#""Bash(npm:*)","Bash(npx:*)""#),
    ]);

    generate(dir, &template_vars, &HashMap::new()).unwrap();

    // project.json should contain substituted values, not placeholders.
    let pj = std::fs::read_to_string(dir.join(".magistr/project.json")).unwrap();
    assert!(pj.contains("my-app"));
    assert!(pj.contains("typescript"));
    assert!(pj.contains("npx tsc --noEmit"));
    assert!(!pj.contains("{{project_name}}"));
    assert!(!pj.contains("{{language}}"));

    // CLAUDE.md should have project name.
    let claude = std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap();
    assert!(claude.contains("my-app"));
    assert!(!claude.contains("{{project_name}}"));

    // conventions.md should have language-specific content.
    let conv = std::fs::read_to_string(dir.join(".claude/rules/conventions.md")).unwrap();
    assert!(conv.contains("TypeScript"));
    assert!(!conv.contains("{{language}}"));
}

#[test]
fn mcp_json_has_project_root() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();

    let template_vars = vars([
        ("project_name", "test-project"),
        ("project_description", ""),
        ("language", "rust"),
        ("framework", "none"),
        ("package_manager", "cargo"),
        ("test_framework", "cargo-test"),
        ("linter", "clippy"),
        ("formatter", "rustfmt"),
        ("check_cmd", "cargo check"),
        ("test_cmd", "cargo test"),
        ("lint_cmd", "cargo clippy"),
        ("format_cmd", "cargo fmt"),
        ("format_extensions", r#""rs""#),
        ("companion_pattern", "tests/{name}.rs"),
        ("test_dir", "tests/"),
        ("state_prefix", "test-project"),
        ("reference_repos", ""),
        ("allowed_tools", ""),
    ]);

    generate(dir, &template_vars, &HashMap::new()).unwrap();

    let mcp = std::fs::read_to_string(dir.join(".mcp.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&mcp).unwrap();

    let args = parsed["mcpServers"]["workbench-mcp"]["args"]
        .as_array()
        .expect("args should be an array");

    // The workspace root is always the last argument — "." on purpose: a
    // committed .mcp.json must carry no path from the machine that ran init.
    let root = args.last().and_then(|v| v.as_str()).unwrap();
    assert_eq!(root, ".", "the workspace root is portable, got: {root}");
    let magistr_args = parsed["mcpServers"]["magistr"]["args"]
        .as_array()
        .expect("magistr args");
    assert_eq!(
        magistr_args.last().and_then(|v| v.as_str()),
        Some("."),
        "magistr's own root is portable too"
    );
    assert!(mcp.ends_with('\n'), ".mcp.json ends with a newline");
}

#[test]
fn generate_preserves_existing_claude_md_and_roadmap() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();

    // Pre-create user-edited files
    std::fs::write(dir.join("CLAUDE.md"), "# My custom content").unwrap();

    let template_vars = vars([
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
    ]);

    let written = generate(dir, &template_vars, &HashMap::new()).unwrap();

    // CLAUDE.md is user-owned (NeverOverwrite); the only write is the
    // think-and-ship section appended between markers, which is reported.
    assert!(written.contains(&"CLAUDE.md".to_string()));
    // Should write fewer files (preserved files excluded)
    assert!(
        written.len() >= 18,
        "Expected at least 18 files, got {}",
        written.len()
    );

    // Existing content should be preserved, with the section appended after it
    let claude = std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap();
    assert!(claude.starts_with("# My custom content\n\n<!-- think-and-ship -->"));
    assert!(claude.trim_end().ends_with("<!-- /think-and-ship -->"));

    // Other scaffold files should still be created
    assert!(dir.join(".magistr/project.json").is_file());
}

// ── AI-generated override tests ────────────────────────────────

#[test]
fn generated_override_replaces_static_template_content() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    let tv = test_vars();

    let mut overrides = HashMap::new();
    overrides.insert(
        ".claude/rules/conventions.md".to_string(),
        "# AI-generated conventions\n\nCustom content here.".to_string(),
    );

    generate(dir, &tv, &overrides).unwrap();

    let conventions = std::fs::read_to_string(dir.join(".claude/rules/conventions.md")).unwrap();
    assert_eq!(
        conventions,
        "# AI-generated conventions\n\nCustom content here."
    );
}

#[test]
fn roadmap_not_scaffolded() {
    // Roadmap creation is handled by `magistr roadmap generate` (MCP tools),
    // NOT by scaffolding. Verify that scaffold does not create any roadmap file.
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    let tv = test_vars();

    generate(dir, &tv, &HashMap::new()).unwrap();

    assert!(
        !dir.join("ROADMAP.md").exists(),
        "ROADMAP.md should not be scaffolded"
    );
    assert!(
        !dir.join(".magistr-roadmap.yml").exists(),
        ".magistr-roadmap.yml should not be scaffolded"
    );
}

#[test]
fn generated_override_for_claude_md_respects_preserve() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    let tv = test_vars();

    // Pre-create CLAUDE.md
    std::fs::write(dir.join("CLAUDE.md"), "# My hand-crafted CLAUDE.md").unwrap();

    // AI generates a different one
    let mut overrides = HashMap::new();
    overrides.insert(
        "CLAUDE.md".to_string(),
        "# AI-generated CLAUDE.md".to_string(),
    );

    generate(dir, &tv, &overrides).unwrap();

    // Existing CLAUDE.md should be preserved (NeverOverwrite); the only
    // addition is the think-and-ship section appended after the user's text.
    let on_disk = std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap();
    assert!(on_disk.starts_with("# My hand-crafted CLAUDE.md\n\n<!-- think-and-ship -->"));
    assert!(!on_disk.contains("AI-generated"));
}

// ── Preview / dry-run tests ────────────────────────────────────

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
        ("reference_repos", ""),
        ("allowed_tools", r#""Bash(cargo:*)""#),
        ("project_type", "cli"),
    ])
}

#[test]
fn preview_empty_dir_all_creates() {
    let tmp = tempfile::TempDir::new().unwrap();
    let result = preview(tmp.path(), &test_vars(), &HashMap::new()).unwrap();

    // All files should be "create" in an empty directory
    assert!(
        result.creates() > 10,
        "empty dir preview should create many files, got: {}",
        result.creates()
    );
    let _total_creates = result.creates();
    assert_eq!(result.overwrites(), 0);
    assert_eq!(result.unchanged(), 0);
    assert_eq!(result.preserved(), 0);

    // No diffs for create actions
    for f in &result.files {
        assert_eq!(f.action, FileAction::Create);
        assert!(f.diff.is_none());
        assert!(f.existing_content.is_none());
    }
}

#[test]
fn preview_existing_scaffold_shows_unchanged() {
    let tmp = tempfile::TempDir::new().unwrap();
    let tv = test_vars();

    // First, generate the scaffold
    generate(tmp.path(), &tv, &HashMap::new()).unwrap();

    // Preview should show all as unchanged (except preserved files)
    let result = preview(tmp.path(), &tv, &HashMap::new()).unwrap();

    assert_eq!(result.creates(), 0);
    assert_eq!(result.overwrites(), 0);
    // CLAUDE.md and project.json are preserved (NeverOverwrite).
    assert_eq!(result.preserved(), 2);
    // Everything else is unchanged (total minus preserved minus creates)
    assert!(
        result.unchanged() > 10,
        "existing scaffold should show most files unchanged, got: {}",
        result.unchanged()
    );
}

#[test]
fn preview_preserves_user_edited_files() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();

    // Create user-edited CLAUDE.md
    std::fs::write(dir.join("CLAUDE.md"), "# My custom content").unwrap();

    let result = preview(dir, &test_vars(), &HashMap::new()).unwrap();

    let claude = result.files.iter().find(|f| f.path == "CLAUDE.md").unwrap();
    assert_eq!(claude.action, FileAction::Preserved);
    assert!(claude.diff.is_none());
}

#[test]
fn preview_detects_modified_files() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    let tv = test_vars();

    // Generate scaffold first
    generate(dir, &tv, &HashMap::new()).unwrap();

    // Modify a file
    std::fs::write(dir.join(".claude/settings.json"), r#"{"modified": true}"#).unwrap();

    let result = preview(dir, &tv, &HashMap::new()).unwrap();

    let settings = result
        .files
        .iter()
        .find(|f| f.path == ".claude/settings.json")
        .unwrap();
    assert_eq!(settings.action, FileAction::Preserved);
    assert!(settings.diff.is_none());
    assert_eq!(settings.new_content, r#"{"modified": true}"#);
}

#[test]
fn preview_does_not_write_files() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();

    // Preview in an empty directory
    let _result = preview(dir, &test_vars(), &HashMap::new()).unwrap();

    // No files should have been created
    assert!(!dir.join(".claude").exists());
    assert!(!dir.join("CLAUDE.md").exists());
}

#[test]
fn unified_diff_shows_changes() {
    let old = "line1\nline2\nline3\n";
    let new = "line1\nmodified\nline3\n";
    let diff = super::unified_diff("test.txt", old, new);

    assert!(diff.contains("--- a/test.txt"));
    assert!(diff.contains("+++ b/test.txt"));
    assert!(diff.contains("-line2"));
    assert!(diff.contains("+modified"));
}

#[test]
fn preview_summary_counts() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    let tv = test_vars();

    // Generate, then modify one file, add user files
    generate(dir, &tv, &HashMap::new()).unwrap();
    // Modify settings.json to get an "overwrite"
    std::fs::write(dir.join(".claude/rules/tools.md"), "# modified").unwrap();

    let result = preview(dir, &tv, &HashMap::new()).unwrap();

    // CLAUDE.md and project.json preserved, 1 overwrite, rest unchanged.
    assert_eq!(result.preserved(), 2);
    assert_eq!(result.overwrites(), 1);
    assert_eq!(result.creates(), 0);
    let total = result.creates() + result.overwrites() + result.unchanged() + result.preserved();
    assert!(
        total > 10,
        "total preview files should match scaffold output, got: {}",
        total
    );
}

#[test]
fn apply_template_plugins_returns_empty_without_plugins() {
    let tmp = tempfile::TempDir::new().unwrap();
    let vars = test_vars().into_iter().collect::<HashMap<_, _>>();

    // Without any installed plugins, should return empty
    let result = apply_template_plugins(tmp.path(), &vars);
    assert!(result.is_empty());
}

// ── MCP merge tests ────────────────────────────────────────────

#[test]
fn merge_mcp_json_creates_file_when_absent() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    let mut tv = test_vars();
    tv.insert("workbench_command".into(), "workbench-mcp".into());
    tv.insert("workbench_args".into(), "\"/tmp/project\"".into());
    tv.insert("project_root".into(), "/tmp/project".into());

    merge_mcp_json(dir, &tv).unwrap();

    let content = std::fs::read_to_string(dir.join(".mcp.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(parsed["mcpServers"]["magistr"].is_object());
    assert!(parsed["mcpServers"]["workbench-mcp"].is_object());
}

#[test]
fn merge_mcp_json_preserves_user_servers() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();

    // Pre-create .mcp.json with a user-added server.
    let existing = serde_json::json!({
        "mcpServers": {
            "my-custom-server": {
                "command": "my-server",
                "args": ["--port", "3000"],
                "type": "stdio"
            }
        }
    });
    std::fs::write(
        dir.join(".mcp.json"),
        serde_json::to_string_pretty(&existing).unwrap(),
    )
    .unwrap();

    let mut tv = test_vars();
    tv.insert("workbench_command".into(), "workbench-mcp".into());
    tv.insert("workbench_args".into(), "\"/tmp/project\"".into());
    tv.insert("project_root".into(), "/tmp/project".into());

    merge_mcp_json(dir, &tv).unwrap();

    let content = std::fs::read_to_string(dir.join(".mcp.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    // User server preserved
    assert!(parsed["mcpServers"]["my-custom-server"].is_object());
    // Magistr servers added
    assert!(parsed["mcpServers"]["magistr"].is_object());
    assert!(parsed["mcpServers"]["workbench-mcp"].is_object());
}

#[test]
fn merge_mcp_json_is_idempotent() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    let mut tv = test_vars();
    tv.insert("workbench_command".into(), "workbench-mcp".into());
    tv.insert("workbench_args".into(), "\"/tmp/project\"".into());
    tv.insert("project_root".into(), "/tmp/project".into());

    // Run merge twice
    merge_mcp_json(dir, &tv).unwrap();
    let first = std::fs::read_to_string(dir.join(".mcp.json")).unwrap();

    merge_mcp_json(dir, &tv).unwrap();
    let second = std::fs::read_to_string(dir.join(".mcp.json")).unwrap();

    assert_eq!(first, second, "merge should be idempotent");
}

#[test]
fn generate_is_idempotent() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    let tv = test_vars();

    // Run generate twice — should not error.
    let first = generate(dir, &tv, &HashMap::new()).unwrap();
    let second = generate(dir, &tv, &HashMap::new()).unwrap();

    // First run should create files.
    assert!(
        first.len() > 10,
        "first generate should scaffold many files, got: {}",
        first.len()
    );

    // Second run: diff-based update — only merge files (.mcp.json, .gitignore)
    // are returned as "written" (merge files always report as updated). All
    // other files are either unchanged (same content) or preserved (NeverOverwrite).
    assert!(
        second.len() < first.len(),
        "second generate should write fewer files than first ({} vs {}): {second:?}",
        second.len(),
        first.len()
    );

    // Content should be identical.
    let mcp1 = std::fs::read_to_string(dir.join(".mcp.json")).unwrap();
    let pj1 = std::fs::read_to_string(dir.join(".magistr/project.json")).unwrap();

    // Third run — still idempotent (same count as second).
    let third = generate(dir, &tv, &HashMap::new()).unwrap();
    assert_eq!(
        third.len(),
        second.len(),
        "third generate should match second (idempotent), got: {third:?}"
    );

    let mcp3 = std::fs::read_to_string(dir.join(".mcp.json")).unwrap();
    let pj3 = std::fs::read_to_string(dir.join(".magistr/project.json")).unwrap();
    assert_eq!(mcp1, mcp3);
    assert_eq!(pj1, pj3);
}

#[test]
fn remove_scaffold_preserves_user_mcp_entries() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    let tv = test_vars();

    // Generate scaffold.
    generate(dir, &tv, &HashMap::new()).unwrap();

    // Add a user server to .mcp.json.
    let mcp_path = dir.join(".mcp.json");
    let mut doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&mcp_path).unwrap()).unwrap();
    doc["mcpServers"]["my-server"] = serde_json::json!({
        "command": "my-server",
        "args": [],
        "type": "stdio"
    });
    std::fs::write(&mcp_path, serde_json::to_string_pretty(&doc).unwrap()).unwrap();

    // Remove scaffold.
    remove_scaffold(dir).unwrap();

    // .mcp.json should still exist with user server, but without magistr/magistr-scope.
    assert!(mcp_path.exists());
    let remaining: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&mcp_path).unwrap()).unwrap();
    assert!(remaining["mcpServers"]["my-server"].is_object());
    assert!(remaining["mcpServers"]["magistr"].is_null());
    assert!(remaining["mcpServers"]["magistr-scope"].is_null());
}

#[test]
fn remove_scaffold_deletes_mcp_json_when_only_jig_entries() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    let tv = test_vars();

    // Generate scaffold (only magistr servers in .mcp.json).
    generate(dir, &tv, &HashMap::new()).unwrap();
    assert!(dir.join(".mcp.json").exists());

    // Remove scaffold — .mcp.json should be deleted entirely.
    remove_scaffold(dir).unwrap();
    assert!(!dir.join(".mcp.json").exists());
}

#[test]
fn merge_mcp_json_handles_malformed_existing_file() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();

    // Write invalid JSON.
    std::fs::write(dir.join(".mcp.json"), "not valid json").unwrap();

    let mut tv = test_vars();
    tv.insert("workbench_command".into(), "workbench-mcp".into());
    tv.insert("workbench_args".into(), "\"/tmp/project\"".into());
    tv.insert("project_root".into(), "/tmp/project".into());

    // Should recover gracefully.
    merge_mcp_json(dir, &tv).unwrap();

    let content = std::fs::read_to_string(dir.join(".mcp.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(parsed["mcpServers"]["magistr"].is_object());
}

// ── Gitignore merge tests ──────────────────────────────────────

#[test]
fn merge_gitignore_creates_file_when_absent() {
    let tmp = tempfile::TempDir::new().unwrap();
    let tv = test_vars();
    merge_gitignore(tmp.path(), &tv).unwrap();

    let content = std::fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
    assert!(content.contains(GITIGNORE_SECTION_START));
    assert!(content.contains(GITIGNORE_SECTION_END));
    // Rust-specific pattern
    assert!(content.contains("/target/"));
    // magistr ephemeral
    assert!(content.contains(".magistr/scope/"));
    // OS default
    assert!(content.contains(".DS_Store"));
}

#[test]
fn merge_gitignore_preserves_user_patterns() {
    let tmp = tempfile::TempDir::new().unwrap();
    let gi_path = tmp.path().join(".gitignore");
    std::fs::write(&gi_path, "# My custom patterns\nmy-secret/\n*.log\n").unwrap();

    let tv = test_vars();
    merge_gitignore(tmp.path(), &tv).unwrap();

    let content = std::fs::read_to_string(&gi_path).unwrap();
    assert!(content.contains("my-secret/"), "user pattern preserved");
    assert!(content.contains("*.log"), "user pattern preserved");
    assert!(
        content.contains(GITIGNORE_SECTION_START),
        "magistr section added"
    );
    assert!(content.contains("/target/"), "language pattern added");
}

#[test]
fn merge_gitignore_is_idempotent() {
    let tmp = tempfile::TempDir::new().unwrap();
    let tv = test_vars();

    merge_gitignore(tmp.path(), &tv).unwrap();
    let first = std::fs::read_to_string(tmp.path().join(".gitignore")).unwrap();

    merge_gitignore(tmp.path(), &tv).unwrap();
    let second = std::fs::read_to_string(tmp.path().join(".gitignore")).unwrap();

    assert_eq!(first, second, "merge should be idempotent");
}

#[test]
fn merge_gitignore_updates_existing_section() {
    let tmp = tempfile::TempDir::new().unwrap();
    let gi_path = tmp.path().join(".gitignore");

    // Write an old magistr section
    let old = format!(
        "# user stuff\nfoo/\n\n{}\nold-pattern\n{}\n",
        GITIGNORE_SECTION_START, GITIGNORE_SECTION_END
    );
    std::fs::write(&gi_path, &old).unwrap();

    let tv = test_vars();
    merge_gitignore(tmp.path(), &tv).unwrap();

    let content = std::fs::read_to_string(&gi_path).unwrap();
    assert!(content.contains("foo/"), "user content preserved");
    assert!(!content.contains("old-pattern"), "old section replaced");
    assert!(
        content.contains("/target/"),
        "new section has language patterns"
    );
    // Only one pair of markers
    assert_eq!(
        content.matches(GITIGNORE_SECTION_START).count(),
        1,
        "exactly one start marker"
    );
}

#[test]
fn merge_gitignore_skips_duplicates() {
    let tmp = tempfile::TempDir::new().unwrap();
    let gi_path = tmp.path().join(".gitignore");

    // User already has /target/
    std::fs::write(&gi_path, "/target/\n").unwrap();

    let tv = test_vars();
    merge_gitignore(tmp.path(), &tv).unwrap();

    let content = std::fs::read_to_string(&gi_path).unwrap();
    // /target/ should appear only once (in user section, not in magistr section)
    assert_eq!(
        content.matches("/target/").count(),
        1,
        "/target/ should not be duplicated"
    );
}

#[test]
fn merge_gitignore_language_specific() {
    let tmp = tempfile::TempDir::new().unwrap();

    // Python
    let mut tv = test_vars();
    tv.insert("language".into(), "python".into());
    merge_gitignore(tmp.path(), &tv).unwrap();
    let content = std::fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
    assert!(content.contains("__pycache__/"), "python pattern present");
    assert!(
        !content.contains("/target/"),
        "rust pattern absent for python"
    );

    // Rust
    let tmp2 = tempfile::TempDir::new().unwrap();
    let tv2 = test_vars(); // language=rust
    merge_gitignore(tmp2.path(), &tv2).unwrap();
    let content2 = std::fs::read_to_string(tmp2.path().join(".gitignore")).unwrap();
    assert!(content2.contains("/target/"), "rust pattern present");
    assert!(
        !content2.contains("__pycache__"),
        "python pattern absent for rust"
    );
}

#[test]
fn remove_magistr_gitignore_entries_preserves_user() {
    let tmp = tempfile::TempDir::new().unwrap();
    let gi_path = tmp.path().join(".gitignore");

    // Generate with user content
    std::fs::write(&gi_path, "# my patterns\nvendor/\n").unwrap();
    let tv = test_vars();
    merge_gitignore(tmp.path(), &tv).unwrap();

    // Remove magistr section
    remove_magistr_gitignore_entries(tmp.path()).unwrap();

    let content = std::fs::read_to_string(&gi_path).unwrap();
    assert!(content.contains("vendor/"), "user pattern preserved");
    assert!(
        !content.contains(GITIGNORE_SECTION_START),
        "magistr section removed"
    );
}

#[test]
fn remove_magistr_gitignore_entries_deletes_empty_file() {
    let tmp = tempfile::TempDir::new().unwrap();
    let gi_path = tmp.path().join(".gitignore");

    // Generate with no user content
    let tv = test_vars();
    merge_gitignore(tmp.path(), &tv).unwrap();
    assert!(gi_path.exists());

    // Remove magistr section — file should be deleted
    remove_magistr_gitignore_entries(tmp.path()).unwrap();
    assert!(!gi_path.exists(), "empty .gitignore should be deleted");
}

#[test]
fn magistr_dir_gitignore_in_scaffold_files() {
    let tv = test_vars();
    let files = scaffold_files(&tv);
    let magistr_gi = files.iter().find(|f| f.path == ".magistr/.gitignore");
    assert!(
        magistr_gi.is_some(),
        ".magistr/.gitignore should be in scaffold files"
    );
    let content = &magistr_gi.unwrap().content;
    assert!(content.contains("!project.json"), "whitelists project.json");
    assert!(content.contains("!roadmap.yaml"), "whitelists roadmap.yaml");
}

#[test]
fn preview_includes_gitignore() {
    let tmp = tempfile::TempDir::new().unwrap();
    let result = preview(tmp.path(), &test_vars(), &HashMap::new()).unwrap();
    let gi = result.files.iter().find(|f| f.path == ".gitignore");
    assert!(gi.is_some(), ".gitignore should be in preview");
    assert_eq!(gi.unwrap().action, FileAction::Create);
}

#[test]
fn mcp_json_snapshot_with_a_checkout_launch_line() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    let mut tv = test_vars();
    tv.insert("workbench_command".into(), "bun".into());
    tv.insert(
        "workbench_args".into(),
        "\"run\", \"/src/workbench-mcp/src/main.ts\", \"/tmp/project\"".into(),
    );
    tv.insert("project_root".into(), "/tmp/project".into());

    merge_mcp_json(dir, &tv).unwrap();

    let content = std::fs::read_to_string(dir.join(".mcp.json")).unwrap();
    insta::assert_snapshot!("mcp_json", content);
}

#[test]
fn settings_json_snapshot_carries_the_guard_hooks() {
    let files = scaffold_files(&test_vars());
    let settings = files
        .iter()
        .find(|f| f.path == ".claude/settings.json")
        .expect("settings.json is scaffolded");
    insta::assert_snapshot!("settings_json", settings.content);

    let hook = files
        .iter()
        .find(|f| f.path == ".claude/hooks/magistr-guard.sh")
        .expect("the guard hook is scaffolded");
    assert!(hook.executable, "hook scripts must be executable");
    assert!(hook.content.contains("guard \"$event\""));
    assert!(
        hook.content.contains("unrecognized subcommand"),
        "old binaries must fail open"
    );
}

#[test]
fn strip_legacy_mcp_entries_retires_old_navigation_servers() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    std::fs::write(
        dir.join(".mcp.json"),
        r#"{"mcpServers":{"magistr":{"command":"magistr"},"magistr-scope":{"command":"magistr-scope"},"iris":{"command":"iris"},"think-and-ship":{"command":"think-and-ship","args":["serve"]},"mine":{"command":"mine"}}}"#,
    )
    .unwrap();

    strip_legacy_mcp_entries(dir).unwrap();

    let parsed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join(".mcp.json")).unwrap()).unwrap();
    let servers = parsed["mcpServers"].as_object().unwrap();
    assert!(servers.contains_key("magistr"));
    assert!(servers.contains_key("mine"), "user servers are kept");
    assert!(!servers.contains_key("magistr-scope"));
    assert!(!servers.contains_key("iris"));
    assert!(
        !servers.contains_key("think-and-ship"),
        "a standalone think-and-ship server is a second process over the one store (tas-ship-state-two-servers)"
    );
}

#[test]
fn scaffolded_skills_have_discoverable_frontmatter() {
    let files = scaffold_files(&test_vars());
    let skills: Vec<_> = files
        .iter()
        .filter(|file| file.path.ends_with("/SKILL.md"))
        .collect();
    assert!(!skills.is_empty(), "the scaffold must include its skills");

    for skill in skills {
        let name = skill.path.rsplit('/').nth(1).expect("skill directory");
        let (metadata, _) = skill
            .content
            .strip_prefix("---\n")
            .and_then(|body| body.split_once("\n---\n"))
            .unwrap_or_else(|| panic!("{} needs YAML frontmatter to be discovered", skill.path));
        let expected_name = format!("name: {name}");
        assert!(
            metadata.lines().any(|line| line == expected_name),
            "{} needs its directory name in frontmatter",
            skill.path
        );
        let description = metadata
            .lines()
            .find_map(|line| line.strip_prefix("description:"))
            .map(str::trim)
            .filter(|description| !description.is_empty())
            .unwrap_or_else(|| panic!("{} needs a skill description", skill.path));
        assert!(
            name.len() <= 64 && description.chars().count() <= 1024,
            "{} exceeds the skill metadata limits",
            skill.path
        );
    }
}

// Cross-host workflow and relative-reference contracts live in workflow_skills.tests.rs.

#[test]
fn scaffolded_skills_and_rules_do_not_name_the_retired_roadmap_tools_or_plan_file() {
    let files = scaffold_files(&test_vars());
    for f in files.iter().filter(|f| f.path.starts_with(".claude/")) {
        assert!(
            !f.content.contains("magistr_roadmap"),
            "{} still names a retired magistr roadmap tool",
            f.path
        );
        assert!(
            !f.content.contains("current-plan.md"),
            "{} still relies on the plan file; the plan is ship_plan",
            f.path
        );
    }
}

#[test]
fn scaffold_bundles_the_workbench_routing_skill_with_its_source_pin() {
    let files = scaffold_files(&test_vars());
    let skill = files
        .iter()
        .find(|f| f.path == ".claude/skills/workbench-routing/SKILL.md")
        .expect("routing skill scaffolded");
    assert!(skill.content.starts_with("---\nname: workbench-routing"));
    assert!(skill.content.contains(workbench::ROUTING_SKILL_SOURCE));
}

#[test]
fn init_in_the_canonical_skill_repo_preserves_byte_equality() {
    // The workbench-mcp shape: the repo carrying the canonical routing skill
    // (its lint rule R6 demands the dogfood copy be the same bytes). Init
    // must byte-copy the canonical file — no provenance marker, and no stale
    // vendored content.
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    let canonical = "# The canonical routing skill, newer than the vendored copy.\n";
    std::fs::create_dir_all(dir.join("skills/workbench-routing")).unwrap();
    std::fs::write(dir.join("skills/workbench-routing/SKILL.md"), canonical).unwrap();

    let template_vars = vars([
        ("project_name", "workbench-mcp"),
        ("project_description", ""),
        ("language", "typescript"),
        ("framework", "none"),
        ("package_manager", "bun"),
        ("test_framework", "bun-test"),
        ("linter", "biome"),
        ("formatter", "biome"),
        ("check_cmd", "tsc --noEmit"),
        ("test_cmd", "bun test"),
        ("lint_cmd", "biome check ."),
        ("format_cmd", "biome check --write ."),
        ("format_extensions", r#""ts""#),
        ("companion_pattern", "{name}.test.{ext}"),
        ("test_dir", "alongside"),
        ("state_prefix", "workbench-mcp"),
        ("allowed_tools", ""),
    ]);
    generate(dir, &template_vars, &HashMap::new()).unwrap();

    let written = std::fs::read_to_string(dir.join(".claude/skills/workbench-routing/SKILL.md"))
        .expect("dogfood copy written");
    assert_eq!(written, canonical, "byte-equality with the canonical file");
    assert!(
        !written.contains("vendored verbatim"),
        "no provenance marker in the canonical repo"
    );
}

#[test]
fn init_outside_the_canonical_repo_keeps_the_provenance_marker() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    let template_vars = vars([
        ("project_name", "other"),
        ("project_description", ""),
        ("language", "rust"),
        ("framework", "none"),
        ("package_manager", "cargo"),
        ("test_framework", "cargo-test"),
        ("linter", "clippy"),
        ("formatter", "rustfmt"),
        ("check_cmd", "cargo check"),
        ("test_cmd", "cargo test"),
        ("lint_cmd", "cargo clippy"),
        ("format_cmd", "cargo fmt"),
        ("format_extensions", r#""rs""#),
        ("companion_pattern", "{name}.tests.{ext}"),
        ("test_dir", "alongside"),
        ("state_prefix", "other"),
        ("allowed_tools", ""),
    ]);
    generate(dir, &template_vars, &HashMap::new()).unwrap();

    let written = std::fs::read_to_string(dir.join(".claude/skills/workbench-routing/SKILL.md"))
        .expect("skill written");
    assert!(
        written.contains("vendored verbatim"),
        "provenance marker stays where no byte-equality contract exists"
    );
}

#[test]
fn merge_mcp_json_preserves_a_working_tree_entry() {
    // The dogfooding case: the repo's own .mcp.json launches workbench-mcp
    // from the working tree. That is a deliberate choice a re-init must not
    // replace with the installed binary.
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/main.ts"), "// entry").unwrap();
    std::fs::write(
        dir.join(".mcp.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "mcpServers": {
                "workbench-mcp": {
                    "command": "bun",
                    "args": ["run", "src/main.ts", "."],
                    "type": "stdio"
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let mut tv = HashMap::new();
    tv.insert("project_root".into(), ".".into());
    tv.insert("workbench_command".into(), "workbench-mcp".into());
    tv.insert("workbench_args".into(), r#"".""#.into());
    merge_mcp_json(dir, &tv).unwrap();

    let content = std::fs::read_to_string(dir.join(".mcp.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(
        parsed["mcpServers"]["workbench-mcp"]["command"], "bun",
        "the working-tree entry survives: {content}"
    );
    assert_eq!(
        parsed["mcpServers"]["workbench-mcp"]["args"][1],
        "src/main.ts"
    );
    // magistr's own entry is still written beside it.
    assert!(parsed["mcpServers"]["magistr"].is_object());
    assert!(content.ends_with('\n'), "trailing newline");
}

#[test]
fn merge_mcp_json_rewrites_a_stale_absolute_entry() {
    // Init's own old output: absolute args naming the project root (a
    // directory, not a file in the tree). That is staleness, not a choice —
    // a re-init replaces it with the portable form.
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    std::fs::write(
        dir.join(".mcp.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "mcpServers": {
                "workbench-mcp": {
                    "command": "workbench-mcp",
                    "args": [dir.to_string_lossy()],
                    "type": "stdio"
                },
                "magistr": {
                    "command": "magistr",
                    "args": ["mcp-serve", dir.to_string_lossy()],
                    "type": "stdio"
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let mut tv = HashMap::new();
    tv.insert("project_root".into(), ".".into());
    tv.insert("workbench_command".into(), "workbench-mcp".into());
    tv.insert("workbench_args".into(), r#"".""#.into());
    merge_mcp_json(dir, &tv).unwrap();

    let content = std::fs::read_to_string(dir.join(".mcp.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed["mcpServers"]["workbench-mcp"]["args"][0], ".");
    assert_eq!(parsed["mcpServers"]["magistr"]["args"][2], ".");
}

#[test]
fn reset_then_init_keeps_a_working_tree_entry_through_remove_and_merge() {
    // Explicit reset followed by init also preserves working-tree launchers.
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/main.ts"), "// entry").unwrap();
    std::fs::write(
        dir.join(".mcp.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "mcpServers": {
                "workbench-mcp": {
                    "command": "bun",
                    "args": ["run", "src/main.ts", "."],
                    "type": "stdio"
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();

    remove_scaffold(dir).unwrap();

    let mut tv = HashMap::new();
    tv.insert("project_root".into(), ".".into());
    tv.insert("workbench_command".into(), "workbench-mcp".into());
    tv.insert("workbench_args".into(), r#"".""#.into());
    merge_mcp_json(dir, &tv).unwrap();

    let content = std::fs::read_to_string(dir.join(".mcp.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(
        parsed["mcpServers"]["workbench-mcp"]["command"], "bun",
        "the dogfood entry survives reset followed by init: {content}"
    );
}

/// Explicit reset runs `remove_scaffold` before a later `generate`, so the
/// merge is handed a file whose managed block has just been cut out —
/// leaving the blank line that used to separate it. Adding another one
/// per run grew every project's `.gitignore` by a blank line for ever
/// (three inits on the real workbench-mcp repo, three blank lines).
#[test]
fn the_gitignore_separator_does_not_accumulate_across_re_inits() {
    let section = "target/\n.magistr/scope/";
    let mut content = "src/generated/\n".to_string();

    for run in 1..=3 {
        content = merge_gitignore_content(&content, section);
        // What reset does between runs: the managed block is cut out
        // and whatever separated it stays behind.
        let start = content.find(GITIGNORE_SECTION_START).expect("section");
        let end = content.find(GITIGNORE_SECTION_END).expect("end");
        let end_line = content[end..]
            .find('\n')
            .map_or(content.len(), |i| end + i + 1);
        let stripped = format!("{}{}", &content[..start], &content[end_line..]);
        assert!(
            !stripped.contains("\n\n\n"),
            "run {run} left a growing run of blank lines: {stripped:?}"
        );
        content = stripped;
    }
}

#[test]
fn mcp_json_serializes_literal_paths_without_template_injection() {
    let tmp = tempfile::tempdir().unwrap();
    let root = "C:/spaces 雪/with \"quote\"/back\\slash\nliteral";
    let vars = HashMap::from([
        ("project_root".into(), root.into()),
        (
            "workbench_command".into(),
            "/launcher with \"quotes\"".into(),
        ),
        (
            "workbench_args".into(),
            serde_json::to_string(root).unwrap(),
        ),
    ]);
    let (preview, action) = preview_mcp_json(tmp.path(), &vars).unwrap();
    assert_eq!(action, FileAction::Create);
    merge_mcp_json(tmp.path(), &vars).unwrap();
    assert_eq!(
        preview,
        std::fs::read_to_string(tmp.path().join(".mcp.json")).unwrap()
    );
    let doc: serde_json::Value = serde_json::from_str(&preview).unwrap();
    assert_eq!(doc["mcpServers"]["magistr"]["args"][2], root);
    assert_eq!(
        doc["mcpServers"]["workbench-mcp"]["command"],
        "/launcher with \"quotes\""
    );
    assert_eq!(doc["mcpServers"]["workbench-mcp"]["args"][0], root);
}

#[test]
fn portable_defaults_remain_upgradeable_but_custom_launches_survive() {
    let tmp = tempfile::tempdir().unwrap();
    let defaults = HashMap::new();
    merge_mcp_json(tmp.path(), &defaults).unwrap();
    let vars = HashMap::from([("workbench_command".into(), "chosen-launcher".into())]);
    let (proposed, action) = preview_mcp_json(tmp.path(), &vars).unwrap();
    assert_eq!(action, FileAction::Overwrite);
    merge_mcp_json(tmp.path(), &vars).unwrap();
    assert_eq!(
        std::fs::read_to_string(tmp.path().join(".mcp.json")).unwrap(),
        proposed
    );
    let doc: serde_json::Value = serde_json::from_str(&proposed).unwrap();
    assert_eq!(
        doc["mcpServers"]["workbench-mcp"]["command"],
        "chosen-launcher"
    );
    assert_eq!(
        doc["mcpServers"]["workbench-mcp"]["args"],
        serde_json::json!(["."])
    );
    merge_mcp_json(tmp.path(), &defaults).unwrap();
    assert_eq!(
        std::fs::read_to_string(tmp.path().join(".mcp.json")).unwrap(),
        proposed
    );
}

#[test]
fn malformed_explicit_mcp_arguments_fail_before_scaffold_writes() {
    for fragment in ["1", "\"unterminated", "{\"key\":true}"] {
        let tmp = tempfile::tempdir().unwrap();
        let mut vars = test_vars();
        vars.insert("workbench_args".into(), fragment.into());
        assert!(preview(tmp.path(), &vars, &HashMap::new()).is_err());
        assert!(generate(tmp.path(), &vars, &HashMap::new()).is_err());
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    }
}

/// A file that already ends with a blank line gets no second one — the
/// separator is what the merge wants, not one more of them. This is the
/// shape the real workbench-mcp `.gitignore` had.
#[test]
fn a_gitignore_ending_in_a_blank_line_gains_no_second_separator() {
    let merged = merge_gitignore_content("target/\n\n", "node_modules/");
    assert!(
        !merged.contains("\n\n\n"),
        "one separating blank line, not two: {merged:?}"
    );
    assert!(merged.contains(GITIGNORE_SECTION_START));
}

/// `--force` re-init must not delete files magistr never wrote. The
/// doc comment on `remove_scaffold` has always claimed it "preserves
/// user-created files in those directories", but it removed the whole
/// `.claude/` tree — which in the real workbench-mcp repo took ten
/// `speckit-*` skills, a user hook and a user rule file with it.
#[test]
fn remove_scaffold_keeps_files_magistr_never_wrote() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    generate(dir, &test_vars(), &HashMap::new()).unwrap();

    // The user's own skills, hook and rule, beside magistr's.
    let theirs = [
        ".claude/skills/speckit-analyze/SKILL.md",
        ".claude/skills/my-own/SKILL.md",
        ".claude/hooks/their-hook.sh",
        ".claude/rules/their-rule.md",
        ".claude/their-notes.md",
    ];
    for path in theirs {
        let full = dir.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(&full, "the user's own content").unwrap();
    }
    assert!(dir.join(".claude/skills/next/SKILL.md").is_file());

    remove_scaffold(dir).unwrap();

    for path in theirs {
        assert!(
            dir.join(path).is_file(),
            "{path} is the user's and must survive a re-init"
        );
    }
    // Magistr's own files are gone, and so is its own namespace.
    assert!(!dir.join(".claude/skills/next/SKILL.md").exists());
    assert!(!dir.join(".claude/rules/workflow.md").exists());
    assert!(!dir.join(".claude/hooks/magistr-guard.sh").exists());
    assert!(!dir.join(".magistr").exists());
}

/// With nothing of the user's left behind, the emitters' own rule
/// applies: an empty directory magistr created is pruned rather than
/// left as litter.
#[test]
fn remove_scaffold_prunes_the_directories_it_emptied() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    generate(dir, &test_vars(), &HashMap::new()).unwrap();

    remove_scaffold(dir).unwrap();

    assert!(!dir.join(".claude/skills").exists(), "pruned when empty");
    assert!(!dir.join(".claude").exists(), "and the root with it");
}
