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
        vec!["Todo item one", "Todo item two", "Future item"]
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
fn progress_zero_total() {
    let summary = SpecSummary::parse("No checkboxes here");
    assert!((summary.progress()).abs() < f64::EPSILON);
}

#[test]
fn serialization_roundtrip() {
    let summary = SpecSummary {
        total: 10,
        checked: 6,
        unchecked: 4,
        unchecked_items: vec!["Item A".into(), "Item B".into()],
    };
    let json = serde_json::to_string(&summary).unwrap();
    let parsed: SpecSummary = serde_json::from_str(&json).unwrap();
    assert_eq!(summary, parsed);
}
