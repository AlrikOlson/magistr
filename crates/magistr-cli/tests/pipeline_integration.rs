//! Integration tests for the detection → scaffold → config pipeline.
//!
//! Validates that `detect_stack` → `ScaffoldPipeline` → generated files
//! round-trips correctly across multiple language stacks, with and without
//! interview-selection overrides.

use std::path::Path;

use magistr::detect::detect_stack;
use magistr::scaffold::ScaffoldSelections;
use magistr::scaffold::pipeline::ScaffoldPipeline;
use magistr::types::Language;

use tempfile::TempDir;

// ── Helpers ────────────────────────────────────────────────────────

fn write_file(dir: &Path, name: &str, content: &str) {
    if let Some(parent) = Path::new(name).parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir.join(parent)).unwrap();
    }
    std::fs::write(dir.join(name), content).unwrap();
}

fn read_project_json(dir: &Path) -> serde_json::Value {
    let content = std::fs::read_to_string(dir.join(".magistr/project.json")).unwrap();
    // Some stacks emit shell commands with backslash-escaped patterns (e.g.
    // Python's mypy exclude). These are invalid JSON escapes, so we do a
    // lenient parse: escape lone backslashes before feeding to serde_json.
    let sanitized = sanitize_json_backslashes(&content);
    serde_json::from_str(&sanitized)
        .unwrap_or_else(|e| panic!("Invalid project.json: {e}\nContent:\n{content}"))
}

/// Escape lone backslashes that aren't valid JSON escape sequences.
fn sanitize_json_backslashes(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;

    while let Some(ch) = chars.next() {
        if ch == '"' && in_string {
            // Check if this quote is escaped (odd number of preceding backslashes).
            // We already consumed backslashes, so this quote ends the string.
            in_string = false;
            result.push(ch);
        } else if ch == '"' {
            in_string = true;
            result.push(ch);
        } else if ch == '\\' && in_string {
            match chars.peek() {
                Some('"' | '\\' | '/' | 'b' | 'f' | 'n' | 'r' | 't' | 'u') => {
                    result.push(ch);
                    result.push(chars.next().unwrap());
                }
                _ => {
                    // Lone backslash — escape it.
                    result.push('\\');
                    result.push('\\');
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}

#[test]
fn bun_without_lockfile_generates_only_declared_checks_and_explicit_choices() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    write_file(
        dir,
        "package.json",
        r#"{"name":"partial-tools","packageManager":"bun@1.2","scripts":{"test":"bun test","release":"bun build index.ts"}}"#,
    );
    write_file(dir, "index.ts", "export const value = 1;");
    let stack = detect_stack(dir).unwrap();
    assert_eq!(stack.package_manager, magistr::types::PackageManager::Bun);
    assert_eq!(stack.test_framework, magistr::types::TestFramework::Unknown);
    let tree = magistr_core::detection::detect::build_project_tree(dir).unwrap();
    let mut pipeline = ScaffoldPipeline::polyglot(&tree);
    pipeline.selections.quality_gate = Some(magistr::scaffold::research::RecommendedQualityGate {
        check: Some("bun run explicit-check.ts".into()),
        test: None,
        lint: None,
    });
    pipeline.generate(dir).unwrap();
    let pj = read_project_json(dir);
    for gates in [&pj["quality_gate"], &pj["archetypes"][0]["quality_gate"]] {
        assert_eq!(gates["check"], "bun run explicit-check.ts");
        assert_eq!(gates["test"], "bun run test");
        assert_eq!(gates["lint"], "");
    }
    assert_eq!(pj["format"]["command"], "");
}

#[test]
fn missing_project_tools_do_not_enter_generated_command_allowlist() {
    let tmp = TempDir::new().unwrap();
    write_file(
        tmp.path(),
        "package.json",
        r#"{"name":"bare","scripts":{"smoke":"bun run index.ts"}}"#,
    );
    write_file(tmp.path(), "index.ts", "export const value = 1;");
    ScaffoldPipeline::headless(detect_stack(tmp.path()).unwrap())
        .generate(tmp.path())
        .unwrap();
    let pj = read_project_json(tmp.path());
    assert_eq!(
        pj["quality_gate"],
        serde_json::json!({"check":"","test":"","lint":""})
    );
    assert_eq!(pj["format"]["command"], "");
    let config = std::fs::read_to_string(tmp.path().join(".magistr/project.json")).unwrap();
    assert!(!config.contains("npm test"));
    assert!(!config.contains("npm run lint"));
}

// ── Multi-language pipeline: detect → scaffold → verify config ─────

#[test]
fn pipeline_rust_project() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    write_file(
        dir,
        "Cargo.toml",
        "[package]\nname = \"my-rust-api\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\naxum = \"0.7\"\n",
    );
    write_file(dir, "src/main.rs", "fn main() {}");

    let stack = detect_stack(dir).unwrap();
    assert_eq!(stack.language, Language::Rust);
    assert_eq!(stack.project_name, "my-rust-api");

    let pipeline = ScaffoldPipeline::headless(stack);
    let written = pipeline.generate(dir).unwrap();
    assert!(!written.is_empty());

    // Verify project.json has correct detected values.
    let pj = read_project_json(dir);
    assert_eq!(pj["stack"]["language"], "rust");
    assert_eq!(pj["project"]["name"], "my-rust-api");
    assert!(
        pj["quality_gate"]["check"]
            .as_str()
            .unwrap()
            .contains("cargo check")
    );
    assert!(
        pj["quality_gate"]["test"]
            .as_str()
            .unwrap()
            .contains("cargo")
    );
    assert!(
        pj["quality_gate"]["lint"]
            .as_str()
            .unwrap()
            .contains("clippy")
    );

    // CLAUDE.md exists and references the project.
    let claude = std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap();
    assert!(claude.contains("my-rust-api"));
}

#[test]
fn pipeline_python_project() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    write_file(
        dir,
        "pyproject.toml",
        "[project]\nname = \"my-py-api\"\ndependencies = [\"fastapi>=0.100\"]\n\n[tool.ruff]\nline-length = 88\n",
    );

    let stack = detect_stack(dir).unwrap();
    assert_eq!(stack.language, Language::Python);
    assert_eq!(stack.project_name, "my-py-api");

    let pipeline = ScaffoldPipeline::headless(stack);
    let written = pipeline.generate(dir).unwrap();
    assert!(!written.is_empty());

    let pj = read_project_json(dir);
    assert_eq!(pj["stack"]["language"], "python");
    assert_eq!(pj["project"]["name"], "my-py-api");

    // CLAUDE.md exists.
    assert!(dir.join("CLAUDE.md").exists());
}

#[test]
fn pipeline_typescript_project() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    write_file(
        dir,
        "package.json",
        r#"{"name":"my-ts-app","dependencies":{"next":"14","react":"18"},"devDependencies":{"typescript":"5","vitest":"^1"}}"#,
    );
    write_file(dir, "tsconfig.json", "{}");

    let stack = detect_stack(dir).unwrap();
    assert_eq!(stack.language, Language::TypeScript);
    assert_eq!(stack.project_name, "my-ts-app");

    let pipeline = ScaffoldPipeline::headless(stack);
    let written = pipeline.generate(dir).unwrap();
    assert!(!written.is_empty());

    let pj = read_project_json(dir);
    assert_eq!(pj["stack"]["language"], "typescript");
    assert_eq!(pj["project"]["name"], "my-ts-app");
}

#[test]
fn pipeline_go_project() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    write_file(
        dir,
        "go.mod",
        "module example.com/my-go-svc\n\ngo 1.21\n\nrequire (\n\tgithub.com/go-chi/chi/v5 v5.0.10\n)\n",
    );
    write_file(dir, "main.go", "package main");

    let stack = detect_stack(dir).unwrap();
    assert_eq!(stack.language, Language::Go);

    let pipeline = ScaffoldPipeline::headless(stack);
    let written = pipeline.generate(dir).unwrap();
    assert!(!written.is_empty());

    let pj = read_project_json(dir);
    assert_eq!(pj["stack"]["language"], "go");
}

// ── Wizard overrides ───────────────────────────────────────────────

#[test]
fn pipeline_with_selection_overrides_applies_custom_files() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    write_file(
        dir,
        "Cargo.toml",
        "[package]\nname = \"wizard-test\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    );

    let stack = detect_stack(dir).unwrap();

    let mut selections = ScaffoldSelections::default();
    selections.generated_files.insert(
        "CLAUDE.md".to_string(),
        "# Custom CLAUDE.md\nOverridden by the interview".to_string(),
    );
    selections.conventions = vec!["Always use Result".to_string(), "No unwrap".to_string()];

    let pipeline = ScaffoldPipeline::with_selections(stack, selections);

    // Overrides are populated.
    assert_eq!(pipeline.overrides().len(), 1);
    assert!(pipeline.overrides().contains_key("CLAUDE.md"));

    // Template vars include conventions.
    let vars = pipeline.template_vars(dir);
    assert!(
        vars.get("ai_conventions")
            .unwrap()
            .contains("Always use Result")
    );

    let written = pipeline.generate(dir).unwrap();
    assert!(!written.is_empty());

    // Verify the override was applied.
    let claude = std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap();
    assert!(claude.contains("Overridden by the interview"));
}

// ── Preview vs Generate consistency ────────────────────────────────

#[test]
fn pipeline_preview_matches_generate_file_set() {
    let tmp_preview = TempDir::new().unwrap();
    let tmp_generate = TempDir::new().unwrap();

    let cargo_toml =
        "[package]\nname = \"preview-test\"\nversion = \"0.1.0\"\nedition = \"2024\"\n";
    write_file(tmp_preview.path(), "Cargo.toml", cargo_toml);
    write_file(tmp_generate.path(), "Cargo.toml", cargo_toml);

    let stack_p = detect_stack(tmp_preview.path()).unwrap();
    let stack_g = detect_stack(tmp_generate.path()).unwrap();

    let pipeline_p = ScaffoldPipeline::headless(stack_p);
    let pipeline_g = ScaffoldPipeline::headless(stack_g);

    let preview = pipeline_p.preview(tmp_preview.path()).unwrap();
    let generated = pipeline_g.generate(tmp_generate.path()).unwrap();

    // Preview should not write files.
    assert!(!tmp_preview.path().join("CLAUDE.md").exists());
    assert!(!tmp_preview.path().join(".magistr/project.json").exists());

    // Generate should write files.
    assert!(tmp_generate.path().join("CLAUDE.md").exists());
    assert!(tmp_generate.path().join(".magistr/project.json").exists());

    // Preview file count should be in the same ballpark as generate.
    // They may differ slightly due to .mcp.json merge vs create, but the
    // core set should match.
    assert!(
        preview.files.len() >= generated.len() / 2,
        "Preview has {} files, generate has {} — too divergent",
        preview.files.len(),
        generated.len()
    );
}

// ── Re-init idempotency ────────────────────────────────────────────

#[test]
fn pipeline_reinit_preserves_user_edited_claude_md() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    write_file(
        dir,
        "Cargo.toml",
        "[package]\nname = \"reinit-test\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    );

    // First init.
    let stack1 = detect_stack(dir).unwrap();
    let pipeline1 = ScaffoldPipeline::headless(stack1);
    pipeline1.generate(dir).unwrap();

    // Simulate user editing CLAUDE.md.
    let user_content = "# My custom CLAUDE.md\nUser-edited content that should be preserved.\n";
    std::fs::write(dir.join("CLAUDE.md"), user_content).unwrap();

    // Second init (re-scaffold).
    let stack2 = detect_stack(dir).unwrap();
    let pipeline2 = ScaffoldPipeline::headless(stack2);
    pipeline2.generate(dir).unwrap();

    // CLAUDE.md should be preserved (it's in PRESERVE_IF_EXISTS).
    let claude = std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap();
    assert!(
        claude.contains("User-edited content"),
        "CLAUDE.md was overwritten during re-init — user content lost"
    );

    // project.json should still be valid.
    let pj = read_project_json(dir);
    assert_eq!(pj["stack"]["language"], "rust");
}

// ── Config round-trip ──────────────────────────────────────────────

#[test]
fn pipeline_config_round_trip_all_detected_fields() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    write_file(
        dir,
        "Cargo.toml",
        "[package]\nname = \"roundtrip\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\naxum = \"0.7\"\n",
    );
    write_file(dir, "src/main.rs", "fn main() {}");

    let stack = detect_stack(dir).unwrap();
    let pipeline = ScaffoldPipeline::headless(stack.clone());
    pipeline.generate(dir).unwrap();

    let pj = read_project_json(dir);

    // All key detection fields should be present in project.json.
    assert_eq!(
        pj["stack"]["language"],
        stack.language.to_string().to_lowercase()
    );
    assert_eq!(pj["project"]["name"], stack.project_name);
    assert!(pj["quality_gate"]["check"].is_string());
    assert!(pj["quality_gate"]["test"].is_string());
    assert!(pj["quality_gate"]["lint"].is_string());
}
