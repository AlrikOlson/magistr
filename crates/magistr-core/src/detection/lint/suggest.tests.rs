use super::*;
use crate::types::*;

fn rust_stack() -> Stack {
    Stack {
        project_name: "test".into(),
        language: Language::Rust,
        package_manager: PackageManager::Cargo,
        framework: Framework::None,
        test_framework: TestFramework::CargoTest,
        linter: Linter::Clippy,
        formatter: Formatter::Rustfmt,
        workspace: Workspace::None,
        ci_provider: CiProvider::None,
        extra_detections: Vec::new(),
    }
}

fn ts_next_stack() -> Stack {
    Stack {
        project_name: "test".into(),
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

#[test]
fn rust_project_suggests_unwrap_rule() {
    let suggestions = suggest_improvements(&rust_stack(), "# My Rust Project\n");
    assert!(suggestions.iter().any(|s| s.rule.contains("unwrap")));
}

#[test]
fn rust_project_skips_unwrap_if_present() {
    let content = "# My Project\n\nNo unwrap() in library code.\n";
    let suggestions = suggest_improvements(&rust_stack(), content);
    assert!(!suggestions.iter().any(|s| s.rule.contains("unwrap")));
}

#[test]
fn ts_project_suggests_any_rule() {
    let stack = ts_next_stack();
    let suggestions = suggest_improvements(&stack, "# My TS Project\n");
    assert!(suggestions.iter().any(|s| s.rule.contains("any")));
}

#[test]
fn next_project_suggests_server_components() {
    let stack = ts_next_stack();
    let suggestions = suggest_improvements(&stack, "# My Next.js Project\n");
    assert!(
        suggestions
            .iter()
            .any(|s| s.rule.contains("Server Component"))
    );
}

#[test]
fn next_project_skips_if_mentioned() {
    let stack = ts_next_stack();
    let content = "# Project\n\nDefault to Server Components.\nAvoid `any` types.\n";
    let suggestions = suggest_improvements(&stack, content);
    assert!(
        !suggestions
            .iter()
            .any(|s| s.rule.contains("Server Component"))
    );
    assert!(!suggestions.iter().any(|s| s.rule.contains("any")));
}

#[test]
fn python_project_suggests_type_hints() {
    let stack = Stack {
        project_name: "test".into(),
        language: Language::Python,
        package_manager: PackageManager::Pip,
        framework: Framework::Django,
        test_framework: TestFramework::Pytest,
        linter: Linter::Ruff,
        formatter: Formatter::Black,
        workspace: Workspace::None,
        ci_provider: CiProvider::None,
        extra_detections: Vec::new(),
    };
    let suggestions = suggest_improvements(&stack, "# My Python Project\n");
    assert!(suggestions.iter().any(|s| s.rule.contains("type hint")));
}

#[test]
fn go_project_suggests_error_wrapping() {
    let stack = Stack {
        project_name: "test".into(),
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
    let suggestions = suggest_improvements(&stack, "# My Go Project\n");
    assert!(suggestions.iter().any(|s| s.rule.contains("Errorf")));
}

#[test]
fn general_suggests_commit_convention() {
    let suggestions = suggest_improvements(&rust_stack(), "# Project\n");
    assert!(
        suggestions
            .iter()
            .any(|s| s.category == "workflow" && s.rule.contains("commit"))
    );
}

#[test]
fn general_skips_commit_if_mentioned() {
    let content = "# Project\n\nUse conventional commit messages.\n";
    let suggestions = suggest_improvements(&rust_stack(), content);
    assert!(
        !suggestions
            .iter()
            .any(|s| s.category == "workflow" && s.rule.contains("commit"))
    );
}

#[test]
fn security_suggestion_when_no_env_mention() {
    let suggestions = suggest_improvements(&rust_stack(), "# Project\n");
    assert!(suggestions.iter().any(|s| s.category == "security"));
}

#[test]
fn no_security_suggestion_when_env_mentioned() {
    let content = "# Project\n\nUse .env files for secrets.\n";
    let suggestions = suggest_improvements(&rust_stack(), content);
    assert!(
        !suggestions
            .iter()
            .any(|s| s.category == "security" && s.rule.contains("secret"))
    );
}

#[test]
fn suggestion_display() {
    let s = Suggestion {
        category: "testing",
        rule: "Write tests".into(),
        rationale: "Tests are good".into(),
    };
    let display = s.to_string();
    assert!(display.contains("[testing]"));
    assert!(display.contains("Write tests"));
    assert!(display.contains("why: Tests are good"));
}
