use super::*;
use crate::types::CiProvider;

fn rust_stack() -> Stack {
    Stack {
        project_name: "test-project".to_string(),
        language: Language::Rust,
        package_manager: PackageManager::Cargo,
        framework: Framework::Axum,
        test_framework: TestFramework::CargoTest,
        linter: Linter::Clippy,
        formatter: Formatter::Rustfmt,
        workspace: Workspace::None,
        ci_provider: CiProvider::None,
        extra_detections: Vec::new(),
    }
}

fn ts_stack() -> Stack {
    Stack {
        project_name: "ts-project".to_string(),
        language: Language::TypeScript,
        package_manager: PackageManager::Npm,
        framework: Framework::Next,
        test_framework: TestFramework::Vitest,
        linter: Linter::Eslint,
        formatter: Formatter::Prettier,
        workspace: Workspace::None,
        ci_provider: CiProvider::None,
        extra_detections: Vec::new(),
    }
}

fn python_stack() -> Stack {
    Stack {
        project_name: "py-project".to_string(),
        language: Language::Python,
        package_manager: PackageManager::Uv,
        framework: Framework::FastApi,
        test_framework: TestFramework::Pytest,
        linter: Linter::Ruff,
        formatter: Formatter::Ruff,
        workspace: Workspace::None,
        ci_provider: CiProvider::None,
        extra_detections: Vec::new(),
    }
}

#[test]
fn analyze_rust_project() {
    let dir = tempfile::TempDir::new().unwrap();
    let stack = rust_stack();
    let memory = analyze(&stack, dir.path());

    assert!(!memory.is_empty());
    assert_eq!(memory.project_name, "test-project");

    // Should have architecture entries
    let arch = memory.by_category(MemoryCategory::Architecture);
    assert!(arch.iter().any(|e| e.text.contains("rust")));
    assert!(arch.iter().any(|e| e.text.contains("axum")));

    // Should have convention entries
    let conv = memory.by_category(MemoryCategory::Conventions);
    assert!(conv.iter().any(|e| e.text.contains("cargo")));
    assert!(conv.iter().any(|e| e.text.contains("clippy")));

    // Should have pattern entries
    let pats = memory.by_category(MemoryCategory::Patterns);
    assert!(pats.iter().any(|e| e.text.contains("?")));

    // Should have mistake entries
    let mistakes = memory.by_category(MemoryCategory::Mistakes);
    assert!(mistakes.iter().any(|e| e.text.contains("unwrap")));
}

#[test]
fn analyze_typescript_project() {
    let dir = tempfile::TempDir::new().unwrap();
    let stack = ts_stack();
    let memory = analyze(&stack, dir.path());

    let pats = memory.by_category(MemoryCategory::Patterns);
    assert!(pats.iter().any(|e| e.text.contains("async/await")));
    // Next.js specific pattern
    assert!(pats.iter().any(|e| e.text.contains("Server Components")));

    let mistakes = memory.by_category(MemoryCategory::Mistakes);
    assert!(mistakes.iter().any(|e| e.text.contains("any")));
}

#[test]
fn analyze_python_project() {
    let dir = tempfile::TempDir::new().unwrap();
    let stack = python_stack();
    let memory = analyze(&stack, dir.path());

    let pats = memory.by_category(MemoryCategory::Patterns);
    assert!(pats.iter().any(|e| e.text.contains("type hints")));

    let mistakes = memory.by_category(MemoryCategory::Mistakes);
    assert!(mistakes.iter().any(|e| e.text.contains("except")));
}

#[test]
fn analyze_detects_directory_structure() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();
    std::fs::create_dir(dir.path().join("tests")).unwrap();

    let stack = rust_stack();
    let memory = analyze(&stack, dir.path());

    let arch = memory.by_category(MemoryCategory::Architecture);
    assert!(arch.iter().any(|e| e.text.contains("src/ + tests/")));
}

#[test]
fn analyze_detects_monorepo() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut stack = rust_stack();
    stack.workspace = Workspace::Cargo;
    let memory = analyze(&stack, dir.path());

    let arch = memory.by_category(MemoryCategory::Architecture);
    assert!(arch.iter().any(|e| e.text.contains("Monorepo")));
}

#[test]
fn analyze_unknown_language_minimal_output() {
    let dir = tempfile::TempDir::new().unwrap();
    let stack = Stack::default();
    let memory = analyze(&stack, dir.path());

    // Should have very few entries for unknown stack
    assert!(memory.by_category(MemoryCategory::Patterns).is_empty());
    assert!(memory.by_category(MemoryCategory::Mistakes).is_empty());
}

#[test]
fn merge_avoids_duplicates() {
    let mut existing = ProjectMemory::new("proj");
    existing.add(MemoryEntry::new(
        MemoryCategory::Patterns,
        "existing pattern",
    ));

    let mut new_entries = ProjectMemory::new("proj");
    new_entries.add(MemoryEntry::new(
        MemoryCategory::Patterns,
        "existing pattern",
    )); // duplicate
    new_entries.add(MemoryEntry::new(MemoryCategory::Patterns, "new pattern")); // new

    merge(&mut existing, &new_entries);

    assert_eq!(existing.len(), 2);
    assert!(
        existing
            .entries
            .iter()
            .any(|e| e.text == "existing pattern")
    );
    assert!(existing.entries.iter().any(|e| e.text == "new pattern"));
}

#[test]
fn merge_into_empty() {
    let mut existing = ProjectMemory::new("proj");
    let mut new_entries = ProjectMemory::new("proj");
    new_entries.add(MemoryEntry::new(MemoryCategory::Patterns, "pattern"));

    merge(&mut existing, &new_entries);
    assert_eq!(existing.len(), 1);
}

#[test]
fn merge_with_empty_new() {
    let mut existing = ProjectMemory::new("proj");
    existing.add(MemoryEntry::new(MemoryCategory::Patterns, "pattern"));

    let new_entries = ProjectMemory::new("proj");
    merge(&mut existing, &new_entries);
    assert_eq!(existing.len(), 1);
}

#[test]
fn analyze_go_project() {
    let dir = tempfile::TempDir::new().unwrap();
    let stack = Stack {
        project_name: "go-project".to_string(),
        language: Language::Go,
        package_manager: PackageManager::Go,
        framework: Framework::Gin,
        test_framework: TestFramework::GoTest,
        linter: Linter::GolangciLint,
        formatter: Formatter::Gofmt,
        workspace: Workspace::None,
        ci_provider: CiProvider::None,
        extra_detections: Vec::new(),
    };
    let memory = analyze(&stack, dir.path());

    let pats = memory.by_category(MemoryCategory::Patterns);
    assert!(pats.iter().any(|e| e.text.contains("err != nil")));

    let mistakes = memory.by_category(MemoryCategory::Mistakes);
    assert!(mistakes.iter().any(|e| e.text.contains("errors")));
}
