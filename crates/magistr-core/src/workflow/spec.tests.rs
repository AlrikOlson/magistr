use super::*;

#[test]
fn parse_empty_spec() {
    let summary = SpecSummary::parse("");
    assert_eq!(summary.total, 0);
    assert_eq!(summary.checked, 0);
    assert_eq!(summary.unchecked, 0);
    assert!(summary.unchecked_items.is_empty());
    assert!(!summary.all_done());
}

#[test]
fn parse_all_checked() {
    let content = "# Roadmap\n- [x] Item one\n- [x] Item two\n- [x] Item three\n";
    let summary = SpecSummary::parse(content);
    assert_eq!(summary.total, 3);
    assert_eq!(summary.checked, 3);
    assert_eq!(summary.unchecked, 0);
    assert!(summary.all_done());
    assert!((summary.progress() - 1.0).abs() < f64::EPSILON);
}

#[test]
fn parse_mixed_items() {
    let content = "\
# Phase 1
- [x] Done item
- [ ] Todo item one
- [x] Another done
- [ ] Todo item two
## Phase 2
- [ ] Future item
";
    let summary = SpecSummary::parse(content);
    assert_eq!(summary.total, 5);
    assert_eq!(summary.checked, 2);
    assert_eq!(summary.unchecked, 3);
    assert!(!summary.all_done());
    assert_eq!(
        summary.unchecked_items,
        vec!["Todo item one", "Todo item two", "Future item",]
    );
}

#[test]
fn parse_uppercase_x() {
    let content = "- [X] Checked with uppercase\n- [ ] Not done\n";
    let summary = SpecSummary::parse(content);
    assert_eq!(summary.checked, 1);
    assert_eq!(summary.unchecked, 1);
}

#[test]
fn parse_indented_items() {
    let content = "  - [x] Indented checked\n  - [ ] Indented unchecked\n";
    let summary = SpecSummary::parse(content);
    assert_eq!(summary.checked, 1);
    assert_eq!(summary.unchecked, 1);
    assert_eq!(summary.unchecked_items, vec!["Indented unchecked"]);
}

#[test]
fn progress_partial() {
    let content = "- [x] A\n- [x] B\n- [ ] C\n- [ ] D\n";
    let summary = SpecSummary::parse(content);
    assert!((summary.progress() - 0.5).abs() < f64::EPSILON);
}

#[test]
fn parse_spec_from_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let spec_path = dir.path().join("ROADMAP.md");
    std::fs::write(&spec_path, "- [x] Done\n- [ ] Todo\n").unwrap();

    let summary = parse_spec(&spec_path).unwrap();
    assert_eq!(summary.total, 2);
    assert_eq!(summary.checked, 1);
    assert_eq!(summary.unchecked, 1);
}

#[test]
fn parse_spec_missing_file() {
    let result = parse_spec(Path::new("/nonexistent/ROADMAP.md"));
    assert!(result.is_err());
}

#[test]
fn spec_summary_from_project_prefers_yaml() {
    let dir = tempfile::TempDir::new().unwrap();

    // Write YAML with 3 tasks (2 checked) at the canonical path
    let yaml_content = r#"
title: Test
phases:
  - id: E1
    name: Phase One
    tasks:
      - text: Task A
        checked: true
      - text: Task B
        checked: true
      - text: Task C
        checked: false
"#;
    std::fs::write(dir.path().join(".magistr-roadmap.yml"), yaml_content).unwrap();

    // Write markdown with different counts (should be ignored)
    let spec_path = dir.path().join("ROADMAP.md");
    std::fs::write(&spec_path, "- [x] One\n- [ ] Two\n").unwrap();

    let summary = spec_summary_from_project(dir.path(), &spec_path).unwrap();
    // Should use YAML (3 total, 2 checked), not markdown (2 total, 1 checked)
    assert_eq!(summary.total, 3);
    assert_eq!(summary.checked, 2);
    assert_eq!(summary.unchecked, 1);
}

#[test]
fn spec_summary_from_project_falls_back_to_markdown() {
    let dir = tempfile::TempDir::new().unwrap();
    // No .magistr/roadmap.yaml — only markdown
    let spec_path = dir.path().join("ROADMAP.md");
    std::fs::write(&spec_path, "- [x] Done\n- [ ] Todo\n- [ ] Also todo\n").unwrap();

    let summary = spec_summary_from_project(dir.path(), &spec_path).unwrap();
    assert_eq!(summary.total, 3);
    assert_eq!(summary.checked, 1);
    assert_eq!(summary.unchecked, 2);
}
