use super::*;

// ── PermissionProfile ──────────────────────────────────────────

#[test]
fn profile_parse_all_variants() {
    assert_eq!(
        "cautious".parse::<PermissionProfile>().unwrap(),
        PermissionProfile::Cautious
    );
    assert_eq!(
        "standard".parse::<PermissionProfile>().unwrap(),
        PermissionProfile::Standard
    );
    assert_eq!(
        "trusted".parse::<PermissionProfile>().unwrap(),
        PermissionProfile::Trusted
    );
    assert_eq!(
        "yolo".parse::<PermissionProfile>().unwrap(),
        PermissionProfile::Yolo
    );
}

#[test]
fn profile_parse_case_insensitive() {
    assert_eq!(
        "CAUTIOUS".parse::<PermissionProfile>().unwrap(),
        PermissionProfile::Cautious
    );
    assert_eq!(
        "Yolo".parse::<PermissionProfile>().unwrap(),
        PermissionProfile::Yolo
    );
}

#[test]
fn profile_parse_unknown_errors() {
    assert!("unknown".parse::<PermissionProfile>().is_err());
}

#[test]
fn profile_display_round_trip() {
    for profile in PermissionProfile::all() {
        let s = profile.to_string();
        let parsed: PermissionProfile = s.parse().unwrap();
        assert_eq!(*profile, parsed);
    }
}

#[test]
fn profile_all_returns_four() {
    assert_eq!(PermissionProfile::all().len(), 4);
}

#[test]
fn profile_descriptions_non_empty() {
    for profile in PermissionProfile::all() {
        assert!(!profile.description().is_empty());
    }
}

// ── Profile Default Actions ────────────────────────────────────

#[test]
fn cautious_asks_for_everything() {
    let p = PermissionProfile::Cautious;
    assert_eq!(p.default_action(RiskLevel::Low), ProfileAction::Ask);
    assert_eq!(p.default_action(RiskLevel::Medium), ProfileAction::Ask);
    assert_eq!(p.default_action(RiskLevel::High), ProfileAction::Ask);
}

#[test]
fn standard_allows_low_asks_rest() {
    let p = PermissionProfile::Standard;
    assert_eq!(p.default_action(RiskLevel::Low), ProfileAction::Allow);
    assert_eq!(p.default_action(RiskLevel::Medium), ProfileAction::Ask);
    assert_eq!(p.default_action(RiskLevel::High), ProfileAction::Ask);
}

#[test]
fn trusted_allows_low_and_medium() {
    let p = PermissionProfile::Trusted;
    assert_eq!(p.default_action(RiskLevel::Low), ProfileAction::Allow);
    assert_eq!(p.default_action(RiskLevel::Medium), ProfileAction::Allow);
    assert_eq!(p.default_action(RiskLevel::High), ProfileAction::Ask);
}

#[test]
fn yolo_allows_everything() {
    let p = PermissionProfile::Yolo;
    assert_eq!(p.default_action(RiskLevel::Low), ProfileAction::Allow);
    assert_eq!(p.default_action(RiskLevel::Medium), ProfileAction::Allow);
    assert_eq!(p.default_action(RiskLevel::High), ProfileAction::Allow);
}

// ── Risk Level ─────────────────────────────────────────────────

#[test]
fn risk_level_ordering() {
    assert!(RiskLevel::Low < RiskLevel::Medium);
    assert!(RiskLevel::Medium < RiskLevel::High);
}

#[test]
fn risk_level_badges_non_empty() {
    assert!(!RiskLevel::Low.badge().is_empty());
    assert!(!RiskLevel::Medium.badge().is_empty());
    assert!(!RiskLevel::High.badge().is_empty());
}

#[test]
fn risk_level_display() {
    assert_eq!(RiskLevel::Low.to_string(), "low");
    assert_eq!(RiskLevel::Medium.to_string(), "medium");
    assert_eq!(RiskLevel::High.to_string(), "high");
}

// ── Risk Classification ────────────────────────────────────────

#[test]
fn classify_read_as_low() {
    let input = serde_json::json!({"file_path": "/src/main.rs"});
    assert_eq!(classify_risk("Read", &input), RiskLevel::Low);
}

#[test]
fn classify_glob_as_low() {
    let input = serde_json::json!({"pattern": "**/*.rs"});
    assert_eq!(classify_risk("Glob", &input), RiskLevel::Low);
}

#[test]
fn classify_grep_as_low() {
    let input = serde_json::json!({"pattern": "TODO"});
    assert_eq!(classify_risk("Grep", &input), RiskLevel::Low);
}

#[test]
fn classify_write_as_medium() {
    let input = serde_json::json!({"file_path": "/src/new.rs", "content": "fn main() {}"});
    assert_eq!(classify_risk("Write", &input), RiskLevel::Medium);
}

#[test]
fn classify_edit_as_medium() {
    let input =
        serde_json::json!({"file_path": "/src/main.rs", "old_string": "a", "new_string": "b"});
    assert_eq!(classify_risk("Edit", &input), RiskLevel::Medium);
}

#[test]
fn classify_bash_ls_as_low() {
    let input = serde_json::json!({"command": "ls -la"});
    assert_eq!(classify_risk("Bash", &input), RiskLevel::Low);
}

#[test]
fn classify_bash_cargo_test_as_low() {
    let input = serde_json::json!({"command": "cargo test --workspace"});
    assert_eq!(classify_risk("Bash", &input), RiskLevel::Low);
}

#[test]
fn classify_bash_rm_as_high() {
    let input = serde_json::json!({"command": "rm -rf /tmp/foo"});
    assert_eq!(classify_risk("Bash", &input), RiskLevel::High);
}

#[test]
fn classify_bash_git_push_as_high() {
    let input = serde_json::json!({"command": "git push --force origin main"});
    assert_eq!(classify_risk("Bash", &input), RiskLevel::High);
}

#[test]
fn classify_bash_sudo_as_high() {
    let input = serde_json::json!({"command": "sudo apt install foo"});
    assert_eq!(classify_risk("Bash", &input), RiskLevel::High);
}

#[test]
fn classify_bash_unknown_as_medium() {
    let input = serde_json::json!({"command": "some-custom-script --arg"});
    assert_eq!(classify_risk("Bash", &input), RiskLevel::Medium);
}

#[test]
fn classify_bash_empty_as_unknown_risk() {
    let input = serde_json::json!({"command": ""});
    assert_eq!(classify_risk("Bash", &input), RiskLevel::High);
}

#[test]
fn classify_mcp_tool_as_medium() {
    let input = serde_json::json!({"query": "search"});
    assert_eq!(
        classify_risk("mcp__magistr-scope__scope_search", &input),
        RiskLevel::Medium
    );
}

#[test]
fn classify_unknown_tool_as_high() {
    let input = serde_json::json!({});
    assert_eq!(classify_risk("SomeNewTool", &input), RiskLevel::High);
}

// ── evaluate ───────────────────────────────────────────────────

#[test]
fn evaluate_standard_allows_read() {
    let decision = evaluate(
        "Read",
        &serde_json::json!({"file_path": "/src/main.rs"}),
        PermissionProfile::Standard,
        &[],
        &[],
    );
    assert_eq!(decision.action, ProfileAction::Allow);
    assert_eq!(decision.risk, RiskLevel::Low);
    assert_eq!(decision.reason, DecisionReason::ProfileDefault);
}

#[test]
fn evaluate_standard_asks_for_write() {
    let decision = evaluate(
        "Write",
        &serde_json::json!({"file_path": "/src/new.rs"}),
        PermissionProfile::Standard,
        &[],
        &[],
    );
    assert_eq!(decision.action, ProfileAction::Ask);
    assert_eq!(decision.risk, RiskLevel::Medium);
}

#[test]
fn evaluate_yolo_allows_everything() {
    let decision = evaluate(
        "Bash",
        &serde_json::json!({"command": "rm -rf /"}),
        PermissionProfile::Yolo,
        &[],
        &[],
    );
    assert_eq!(decision.action, ProfileAction::Allow);
    assert_eq!(decision.risk, RiskLevel::High);
}

#[test]
fn evaluate_auto_allow_rule_overrides_profile() {
    let rules = vec![rules::AutoAllowRule::tool_name("Write", "Allow all writes")];
    let decision = evaluate(
        "Write",
        &serde_json::json!({"file_path": "/src/new.rs"}),
        PermissionProfile::Cautious, // would normally ask
        &rules,
        &[],
    );
    assert_eq!(decision.action, ProfileAction::Allow);
    assert!(matches!(decision.reason, DecisionReason::AutoAllowRule(_)));
}

#[test]
fn evaluate_gate_command_auto_approves() {
    let gates = vec!["cargo test --workspace".to_string()];
    let decision = evaluate(
        "Bash",
        &serde_json::json!({"command": "cargo test --workspace"}),
        PermissionProfile::Cautious,
        &[],
        &gates,
    );
    assert_eq!(decision.action, ProfileAction::Allow);
    assert!(matches!(decision.reason, DecisionReason::BatchApproval(_)));
}

#[test]
fn evaluate_gate_command_no_match() {
    let gates = vec!["cargo test --workspace".to_string()];
    let decision = evaluate(
        "Bash",
        &serde_json::json!({"command": "rm -rf /"}),
        PermissionProfile::Cautious,
        &[],
        &gates,
    );
    assert_eq!(decision.action, ProfileAction::Ask);
}

// ── DecisionReason Display ─────────────────────────────────────

#[test]
fn decision_reason_display() {
    assert_eq!(
        DecisionReason::ProfileDefault.to_string(),
        "profile default"
    );
    assert_eq!(
        DecisionReason::AutoAllowRule("test rule".into()).to_string(),
        "auto-allow rule: test rule"
    );
    assert_eq!(
        DecisionReason::BatchApproval("cargo test".into()).to_string(),
        "batch approval: cargo test"
    );
}
