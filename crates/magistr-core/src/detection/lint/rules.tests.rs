use super::*;

#[test]
fn file_too_long_error() {
    let content = "line\n".repeat(501);
    let diags = check_file_too_long(&content);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Error);
    assert_eq!(diags[0].rule, "too-long");
}

#[test]
fn file_approaching_limit_warning() {
    let content = "line\n".repeat(350);
    let diags = check_file_too_long(&content);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Warning);
}

#[test]
fn file_short_no_diagnostic() {
    let content = "line\n".repeat(100);
    let diags = check_file_too_long(&content);
    assert!(diags.is_empty());
}

#[test]
fn wall_of_text_detected() {
    // 250 words in one paragraph
    let words: String = (0..250).map(|i| format!("word{i} ")).collect();
    let diags = check_wall_of_text(&words);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].rule, "wall-of-text");
}

#[test]
fn wall_of_text_not_triggered_by_short_paragraphs() {
    let content = "Short paragraph one.\n\nShort paragraph two.\n";
    let diags = check_wall_of_text(content);
    assert!(diags.is_empty());
}

#[test]
fn vague_rule_detected() {
    let content = "You should write good code and use best practices.\n";
    let diags = check_vague_rules(content);
    assert!(diags.len() >= 2); // "write good code" and "use best practices"
    assert!(diags.iter().all(|d| d.rule == "vague-rule"));
}

#[test]
fn vague_rule_case_insensitive() {
    let content = "Write Good Code always.\n";
    let diags = check_vague_rules(content);
    assert!(!diags.is_empty());
}

#[test]
fn no_vague_rules_in_specific_content() {
    let content = "Functions must be under 30 lines.\nUse `cargo clippy` for linting.\n";
    let diags = check_vague_rules(content);
    assert!(diags.is_empty());
}

#[test]
fn contradiction_detected() {
    let content = "Always use unwrap for simplicity.\nNever use unwrap in production.\n";
    let diags = check_contradictions(content);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Error);
    assert_eq!(diags[0].rule, "contradiction");
}

#[test]
fn no_contradiction_without_both() {
    let content = "Never use unwrap in production code.\n";
    let diags = check_contradictions(content);
    assert!(diags.is_empty());
}

#[test]
fn duplicate_section_detected() {
    let content = "## Error Handling\n\nDo X.\n\n## Error Handling\n\nDo Y.\n";
    let diags = check_duplicate_sections(content);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].rule, "duplicate-section");
    assert_eq!(diags[0].line, Some(5));
}

#[test]
fn no_duplicate_for_different_headings() {
    let content = "## Error Handling\n\nDo X.\n\n## Testing\n\nDo Y.\n";
    let diags = check_duplicate_sections(content);
    assert!(diags.is_empty());
}

#[test]
fn missing_gates_detected() {
    let content = "# My Project\n\nThis is a project without any test commands.\n";
    let diags = check_no_quality_gates(content);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].rule, "missing-gates");
}

#[test]
fn gates_found_with_cargo_test() {
    let content = "Run `cargo test` to verify.\n";
    let diags = check_no_quality_gates(content);
    assert!(diags.is_empty());
}

#[test]
fn empty_file_error() {
    let diags = check_no_structure("");
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Error);
    assert_eq!(diags[0].rule, "empty-file");
}

#[test]
fn no_structure_warning_for_long_plain_text() {
    let content = (0..15)
        .map(|i| format!("This is line {i} of plain text."))
        .collect::<Vec<_>>()
        .join("\n");
    let diags = check_no_structure(&content);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].rule, "no-structure");
}

#[test]
fn structured_content_ok() {
    let content = "# Project\n\n## Rules\n\n- Rule one\n- Rule two\n";
    let diags = check_no_structure(content);
    assert!(diags.is_empty());
}

#[test]
fn deprecated_pattern_flagged() {
    let content = "Use TSLint for TypeScript linting.\n";
    let diags = check_deprecated_patterns(content);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Info);
}

#[test]
fn no_deprecated_in_clean_content() {
    let content = "Use ESLint with flat config for linting.\n";
    let diags = check_deprecated_patterns(content);
    assert!(diags.is_empty());
}

#[test]
fn run_all_produces_combined_diagnostics() {
    let content = "";
    let diags = run_all(content);
    // Empty file should produce at least the "empty-file" error
    assert!(diags.iter().any(|d| d.rule == "empty-file"));
}
