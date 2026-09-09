use std::io::Write;

use chrono::Utc;

use super::*;
use tempfile::TempDir;

fn sample_entry(tool: &str, allowed: bool, risk: RiskLevel) -> AuditEntry {
    AuditEntry {
        timestamp: Utc::now(),
        tool_name: tool.to_string(),
        risk,
        allowed,
        reason: "profile default".to_string(),
        project: Some("test-project".to_string()),
        input_summary: None,
        actor: None,
    }
}

// ── Append & Load ──────────────────────────────────────────────

#[test]
fn append_and_load_round_trip() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.log");

    let entry = sample_entry("Read", true, RiskLevel::Low);
    append_entry(&path, &entry).unwrap();

    let entries = load_entries(&path).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].tool_name, "Read");
    assert!(entries[0].allowed);
}

#[test]
fn load_nonexistent_returns_empty() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nonexistent.log");
    let entries = load_entries(&path).unwrap();
    assert!(entries.is_empty());
}

#[test]
fn append_creates_parent_dirs() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nested").join("deep").join("test.log");

    let entry = sample_entry("Write", true, RiskLevel::Medium);
    append_entry(&path, &entry).unwrap();

    let entries = load_entries(&path).unwrap();
    assert_eq!(entries.len(), 1);
}

#[test]
fn load_skips_malformed_lines() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.log");

    let entry = sample_entry("Read", true, RiskLevel::Low);
    append_entry(&path, &entry).unwrap();

    // Append a malformed line
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap();
    writeln!(file, "not valid json").unwrap();

    // Append another valid entry
    let entry2 = sample_entry("Write", false, RiskLevel::Medium);
    append_entry(&path, &entry2).unwrap();

    let entries = load_entries(&path).unwrap();
    assert_eq!(entries.len(), 2);
}

#[test]
fn load_skips_empty_lines() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.log");

    let entry = sample_entry("Read", true, RiskLevel::Low);
    append_entry(&path, &entry).unwrap();

    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap();
    writeln!(file).unwrap();
    writeln!(file, "   ").unwrap();

    let entries = load_entries(&path).unwrap();
    assert_eq!(entries.len(), 1);
}

// ── Summary ────────────────────────────────────────────────────

#[test]
fn summarize_counts() {
    let entries = vec![
        sample_entry("Read", true, RiskLevel::Low),
        sample_entry("Read", true, RiskLevel::Low),
        sample_entry("Write", true, RiskLevel::Medium),
        sample_entry("Bash", false, RiskLevel::High),
    ];

    let summary = summarize(&entries);
    assert_eq!(summary.total, 4);
    assert_eq!(summary.allowed, 3);
    assert_eq!(summary.denied, 1);
    assert_eq!(summary.top_allowed_tools[0], ("Read".to_string(), 2));
}

#[test]
fn summarize_empty() {
    let summary = summarize(&[]);
    assert_eq!(summary.total, 0);
    assert_eq!(summary.allowed, 0);
    assert_eq!(summary.denied, 0);
    assert!(summary.top_allowed_tools.is_empty());
}

// ── Trust Upgrade Suggestion ───────────────────────────────────

#[test]
fn suggest_upgrade_with_many_low_risk() {
    let entries: Vec<_> = (0..25)
        .map(|_| sample_entry("Read", true, RiskLevel::Low))
        .collect();

    let suggestion = suggest_trust_upgrade(&entries);
    assert!(suggestion.is_some());
    assert!(suggestion.unwrap().contains("--trust standard"));
}

#[test]
fn no_suggest_with_few_entries() {
    let entries: Vec<_> = (0..5)
        .map(|_| sample_entry("Read", true, RiskLevel::Low))
        .collect();

    assert!(suggest_trust_upgrade(&entries).is_none());
}

#[test]
fn no_suggest_when_mostly_denied() {
    let entries: Vec<_> = (0..25)
        .map(|_| sample_entry("Bash", false, RiskLevel::High))
        .collect();

    assert!(suggest_trust_upgrade(&entries).is_none());
}

// ── AuditEntry from_decision ───────────────────────────────────

#[test]
fn from_decision_creates_entry() {
    let entry = AuditEntry::from_decision(
        "Read",
        RiskLevel::Low,
        ProfileAction::Allow,
        &DecisionReason::ProfileDefault,
        Some("my-project"),
        Some("file_path: /src/main.rs"),
        Some("dev@example.com"),
    );

    assert_eq!(entry.tool_name, "Read");
    assert!(entry.allowed);
    assert_eq!(entry.risk, RiskLevel::Low);
    assert_eq!(entry.project, Some("my-project".to_string()));
    assert_eq!(entry.actor, Some("dev@example.com".to_string()));
}

#[test]
fn from_decision_denied() {
    let entry = AuditEntry::from_decision(
        "Bash",
        RiskLevel::High,
        ProfileAction::Ask, // "Ask" means it wasn't auto-allowed
        &DecisionReason::ProfileDefault,
        None,
        None,
        None,
    );

    // Ask != Allow, so allowed should be false
    assert!(!entry.allowed);
    assert_eq!(entry.risk, RiskLevel::High);
}

// ── Serialization ──────────────────────────────────────────────

#[test]
fn audit_entry_json_round_trip() {
    let entry = sample_entry("Edit", true, RiskLevel::Medium);
    let json = serde_json::to_string(&entry).unwrap();
    let back: AuditEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(entry.tool_name, back.tool_name);
    assert_eq!(entry.allowed, back.allowed);
}

#[test]
fn audit_entry_optional_fields_omitted() {
    let entry = AuditEntry {
        timestamp: Utc::now(),
        tool_name: "Read".to_string(),
        risk: RiskLevel::Low,
        allowed: true,
        reason: "test".to_string(),
        project: None,
        input_summary: None,
        actor: None,
    };
    let json = serde_json::to_string(&entry).unwrap();
    assert!(!json.contains("project"));
    assert!(!json.contains("input_summary"));
    assert!(!json.contains("actor"));
}

#[test]
fn audit_entry_actor_round_trip() {
    let mut entry = sample_entry("Edit", true, RiskLevel::Medium);
    entry.actor = Some("Dev One <dev@example.com>".to_string());
    let json = serde_json::to_string(&entry).unwrap();
    let back: AuditEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(back.actor, entry.actor);
}

#[test]
fn pre_actor_entry_deserializes_without_actor() {
    // A log line written before the actor field existed.
    let json = r#"{"timestamp":"2026-08-01T12:00:00Z","tool_name":"Read","risk":"low","allowed":true,"reason":"profile default"}"#;
    let entry: AuditEntry = serde_json::from_str(json).unwrap();
    assert_eq!(entry.tool_name, "Read");
    assert_eq!(entry.actor, None);
}

// ── Retention on the log file ──────────────────────────────────

#[test]
fn apply_retention_unlimited_is_noop() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.log");
    append_entry(&path, &sample_entry("Read", true, RiskLevel::Low)).unwrap();

    let pruned = apply_retention(&path, &AuditRetention::default()).unwrap();
    assert_eq!(pruned, 0);
    assert_eq!(load_entries(&path).unwrap().len(), 1);
}

#[test]
fn apply_retention_missing_file_is_noop() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nonexistent.log");
    let retention = AuditRetention {
        days: Some(1),
        max_entries: None,
    };
    assert_eq!(apply_retention(&path, &retention).unwrap(), 0);
    assert!(!path.exists());
}

#[test]
fn apply_retention_caps_entry_count() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.log");
    for i in 0..5 {
        append_entry(
            &path,
            &sample_entry(&format!("Tool{i}"), true, RiskLevel::Low),
        )
        .unwrap();
    }

    let retention = AuditRetention {
        days: None,
        max_entries: Some(2),
    };
    let pruned = apply_retention(&path, &retention).unwrap();
    assert_eq!(pruned, 3);

    let kept = load_entries(&path).unwrap();
    assert_eq!(kept.len(), 2);
    // The newest entries survive.
    assert_eq!(kept[0].tool_name, "Tool3");
    assert_eq!(kept[1].tool_name, "Tool4");
}

#[test]
fn apply_retention_drops_old_entries() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.log");

    let mut old = sample_entry("Old", true, RiskLevel::Low);
    old.timestamp = Utc::now() - chrono::Duration::days(40);
    append_entry(&path, &old).unwrap();
    append_entry(&path, &sample_entry("Fresh", true, RiskLevel::Low)).unwrap();

    let retention = AuditRetention {
        days: Some(30),
        max_entries: None,
    };
    let pruned = apply_retention(&path, &retention).unwrap();
    assert_eq!(pruned, 1);

    let kept = load_entries(&path).unwrap();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].tool_name, "Fresh");
}
