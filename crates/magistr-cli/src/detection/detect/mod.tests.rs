use super::*;
use crate::plugin;
use tempfile::TempDir;

// ── Plugin detection rule matching ────────────────────────────────

#[test]
fn matches_rule_by_file_pattern() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("custom.config"), "").unwrap();

    let rule = plugin::DetectionRule {
        name: "custom-tool".into(),
        file_patterns: vec!["custom.config".into()],
        content_patterns: vec![],
        confidence: 90,
        category: "tool".into(),
    };

    assert!(matches_detection_rule(tmp.path(), &rule));
}

#[test]
fn no_match_when_file_missing() {
    let tmp = TempDir::new().unwrap();

    let rule = plugin::DetectionRule {
        name: "custom-tool".into(),
        file_patterns: vec!["nonexistent.config".into()],
        content_patterns: vec![],
        confidence: 90,
        category: "tool".into(),
    };

    assert!(!matches_detection_rule(tmp.path(), &rule));
}

#[test]
fn matches_rule_with_content_pattern() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("config.yml"), "framework: internal-x\n").unwrap();

    let rule = plugin::DetectionRule {
        name: "internal-x".into(),
        file_patterns: vec!["config.yml".into()],
        content_patterns: vec!["internal-x".into()],
        confidence: 85,
        category: "framework".into(),
    };

    assert!(matches_detection_rule(tmp.path(), &rule));
}

#[test]
fn no_match_when_content_pattern_missing() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("config.yml"), "framework: other\n").unwrap();

    let rule = plugin::DetectionRule {
        name: "internal-x".into(),
        file_patterns: vec!["config.yml".into()],
        content_patterns: vec!["internal-x".into()],
        confidence: 85,
        category: "framework".into(),
    };

    assert!(!matches_detection_rule(tmp.path(), &rule));
}

#[test]
fn no_match_when_both_empty() {
    let tmp = TempDir::new().unwrap();

    let rule = plugin::DetectionRule {
        name: "nothing".into(),
        file_patterns: vec![],
        content_patterns: vec![],
        confidence: 50,
        category: "tool".into(),
    };

    assert!(!matches_detection_rule(tmp.path(), &rule));
}
