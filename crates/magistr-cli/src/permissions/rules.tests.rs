use super::*;
use serde_json::json;

// ── RuleMatcher: ToolName ──────────────────────────────────────

#[test]
fn tool_name_matches_exact() {
    let rule = AutoAllowRule::tool_name("Read", "allow reads");
    assert!(rule.matches("Read", &json!({"file_path": "/src/main.rs"})));
}

#[test]
fn tool_name_no_match_different() {
    let rule = AutoAllowRule::tool_name("Read", "allow reads");
    assert!(!rule.matches("Write", &json!({"file_path": "/src/main.rs"})));
}

// ── RuleMatcher: BashPrefix ────────────────────────────────────

#[test]
fn bash_prefix_matches_command() {
    let rule = AutoAllowRule::bash_prefix("cargo ", "allow cargo");
    assert!(rule.matches("Bash", &json!({"command": "cargo test --workspace"})));
}

#[test]
fn bash_prefix_no_match_different_command() {
    let rule = AutoAllowRule::bash_prefix("cargo ", "allow cargo");
    assert!(!rule.matches("Bash", &json!({"command": "npm test"})));
}

#[test]
fn bash_prefix_no_match_non_bash() {
    let rule = AutoAllowRule::bash_prefix("cargo ", "allow cargo");
    assert!(!rule.matches("Read", &json!({"command": "cargo test"})));
}

// ── RuleMatcher: BashContains ──────────────────────────────────

#[test]
fn bash_contains_matches_substring() {
    let rule = AutoAllowRule::bash_contains("--workspace", "workspace cmds");
    assert!(rule.matches("Bash", &json!({"command": "cargo test --workspace"})));
}

#[test]
fn bash_contains_no_match() {
    let rule = AutoAllowRule::bash_contains("--workspace", "workspace cmds");
    assert!(!rule.matches("Bash", &json!({"command": "cargo test"})));
}

// ── RuleMatcher: PathPrefix ────────────────────────────────────

#[test]
fn path_prefix_matches_read() {
    let rule = AutoAllowRule::path_prefix("/src/", "allow src reads");
    assert!(rule.matches("Read", &json!({"file_path": "/src/main.rs"})));
}

#[test]
fn path_prefix_matches_write() {
    let rule = AutoAllowRule::path_prefix("/src/", "allow src writes");
    assert!(rule.matches("Write", &json!({"file_path": "/src/new.rs", "content": ""})));
}

#[test]
fn path_prefix_matches_edit() {
    let rule = AutoAllowRule::path_prefix("/src/", "allow src edits");
    assert!(rule.matches(
        "Edit",
        &json!({"file_path": "/src/main.rs", "old_string": "", "new_string": ""})
    ));
}

#[test]
fn path_prefix_no_match_wrong_path() {
    let rule = AutoAllowRule::path_prefix("/src/", "allow src");
    assert!(!rule.matches("Read", &json!({"file_path": "/tests/test.rs"})));
}

#[test]
fn path_prefix_no_match_bash() {
    let rule = AutoAllowRule::path_prefix("/src/", "allow src");
    assert!(!rule.matches("Bash", &json!({"file_path": "/src/main.rs"})));
}

// ── RuleMatcher: AllReads ──────────────────────────────────────

#[test]
fn all_reads_matches_read_tools() {
    let rule = AutoAllowRule::all_reads("allow all reads");
    assert!(rule.matches("Read", &json!({})));
    assert!(rule.matches("Glob", &json!({})));
    assert!(rule.matches("Grep", &json!({})));
    assert!(rule.matches("WebSearch", &json!({})));
    assert!(rule.matches("WebFetch", &json!({})));
}

#[test]
fn all_reads_no_match_write() {
    let rule = AutoAllowRule::all_reads("allow all reads");
    assert!(!rule.matches("Write", &json!({})));
    assert!(!rule.matches("Edit", &json!({})));
    assert!(!rule.matches("Bash", &json!({})));
}

// ── RuleMatcher: AllOfTool ─────────────────────────────────────

#[test]
fn all_of_tool_matches_any_input() {
    let rule = AutoAllowRule::all_of_tool("Edit", "allow all edits");
    assert!(rule.matches("Edit", &json!({"file_path": "/anything"})));
    assert!(rule.matches("Edit", &json!({})));
}

#[test]
fn all_of_tool_no_match_different_tool() {
    let rule = AutoAllowRule::all_of_tool("Edit", "allow all edits");
    assert!(!rule.matches("Write", &json!({})));
}

// ── Quality gate rules ─────────────────────────────────────────

#[test]
fn quality_gate_rules_creates_rules() {
    let rules = rules_from_quality_gates(
        "cargo check --workspace",
        "cargo test --workspace",
        "cargo clippy --workspace -- -D warnings",
    );
    assert_eq!(rules.len(), 3);
    assert!(rules[0].matches("Bash", &json!({"command": "cargo check --workspace"})));
    assert!(rules[1].matches("Bash", &json!({"command": "cargo test --workspace"})));
    assert!(rules[2].matches(
        "Bash",
        &json!({"command": "cargo clippy --workspace -- -D warnings"})
    ));
}

#[test]
fn quality_gate_rules_skips_empty() {
    let rules = rules_from_quality_gates("cargo check", "", "cargo clippy");
    assert_eq!(rules.len(), 2);
}

// ── Serialization ──────────────────────────────────────────────

#[test]
fn rule_serialization_round_trip() {
    let rule = AutoAllowRule::bash_prefix("cargo ", "allow cargo commands");
    let json = serde_json::to_string(&rule).unwrap();
    let deserialized: AutoAllowRule = serde_json::from_str(&json).unwrap();
    assert_eq!(rule, deserialized);
}

#[test]
fn all_matcher_variants_serialize() {
    let rules = vec![
        AutoAllowRule::tool_name("Read", "read"),
        AutoAllowRule::bash_prefix("cargo", "cargo"),
        AutoAllowRule::bash_contains("test", "test"),
        AutoAllowRule::path_prefix("/src/", "src"),
        AutoAllowRule::all_reads("reads"),
        AutoAllowRule::all_of_tool("Edit", "edit"),
    ];
    for rule in &rules {
        let json = serde_json::to_string(rule).unwrap();
        let back: AutoAllowRule = serde_json::from_str(&json).unwrap();
        assert_eq!(*rule, back);
    }
}
