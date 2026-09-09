use super::*;
use crate::scaffold::vars;

#[test]
fn resolve_with_no_ai_returns_base_only() {
    let v = vars([("language", "rust")]);
    let resolved = resolve_conventions(&v);

    assert!(!resolved.base.is_empty());
    assert!(resolved.ai_conventions.is_empty());
    assert!(resolved.ai_tips.is_empty());

    let doc = resolved.full_document();
    assert_eq!(doc, resolved.base);
    assert!(!doc.contains("Project-Specific Conventions"));
    assert!(!doc.contains("AI Agent Tips"));
}

#[test]
fn resolve_with_ai_conventions_appends_section() {
    let v = vars([
        ("language", "rust"),
        ("ai_conventions", "- Use thiserror\n- No unwrap"),
    ]);
    let resolved = resolve_conventions(&v);
    let doc = resolved.full_document();

    assert!(doc.starts_with(resolved.base));
    assert!(doc.contains("## Project-Specific Conventions"));
    assert!(doc.contains("- Use thiserror"));
    assert!(doc.contains("- No unwrap"));
}

#[test]
fn resolve_with_ai_tips_appends_section() {
    let v = vars([("language", "python"), ("ai_tips", "- Enable ruff in CI")]);
    let resolved = resolve_conventions(&v);
    let doc = resolved.full_document();

    assert!(doc.contains("## AI Agent Tips"));
    assert!(doc.contains("- Enable ruff in CI"));
}

#[test]
fn resolve_with_both_ai_fields() {
    let v = vars([
        ("language", "go"),
        ("ai_conventions", "- Use errgroup"),
        ("ai_tips", "- Run go vet first"),
    ]);
    let doc = resolve_conventions(&v).full_document();

    assert!(doc.contains("## Project-Specific Conventions"));
    assert!(doc.contains("- Use errgroup"));
    assert!(doc.contains("## AI Agent Tips"));
    assert!(doc.contains("- Run go vet first"));

    // AI conventions should come before tips
    let conv_pos = doc.find("Project-Specific Conventions").unwrap();
    let tips_pos = doc.find("AI Agent Tips").unwrap();
    assert!(conv_pos < tips_pos);
}

#[test]
fn resolve_unknown_language_uses_default() {
    let v = vars([("language", "brainfuck")]);
    let resolved = resolve_conventions(&v);
    // Should not panic, returns default conventions
    assert!(!resolved.base.is_empty());
}

#[test]
fn resolve_missing_language_uses_default() {
    let v = std::collections::HashMap::new();
    let resolved = resolve_conventions(&v);
    assert!(!resolved.base.is_empty());
}

#[test]
fn all_languages_resolve_without_panic() {
    for lang in [
        "rust",
        "typescript",
        "javascript",
        "python",
        "go",
        "java",
        "ruby",
        "elixir",
        "kotlin",
        "swift",
        "cpp",
        "c++",
        "csharp",
        "c#",
        "unknown",
    ] {
        let v = vars([("language", lang)]);
        let resolved = resolve_conventions(&v);
        assert!(
            !resolved.base.is_empty(),
            "conventions for {lang} should not be empty"
        );
    }
}
