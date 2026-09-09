use super::*;

use serde_json::json;

#[test]
fn a_bash_call_carries_its_command_and_high_risk() {
    let request = policy_request_for("Bash", &json!({"command": "rm -rf /tmp/x"}), None);
    assert_eq!(request.tool.as_deref(), Some("Bash"));
    assert_eq!(request.command.as_deref(), Some("rm -rf /tmp/x"));
    assert_eq!(request.risk, Some(RiskTier::High));
    assert!(request.paths.is_empty());
}

#[test]
fn a_write_call_carries_its_path() {
    let request = policy_request_for("Write", &json!({"file_path": "src/main.rs"}), None);
    assert_eq!(request.paths, vec!["src/main.rs".to_string()]);
    assert_eq!(request.risk, Some(RiskTier::Medium));
}

#[test]
fn every_path_shaped_field_is_collected() {
    let input = json!({
        "path": "a.rs",
        "notebook_path": "b.ipynb",
        "paths": ["c.rs", "d.rs"],
    });
    let request = policy_request_for("Edit", &input, None);
    assert_eq!(request.paths, vec!["a.rs", "b.ipynb", "c.rs", "d.rs"]);
}

#[test]
fn the_model_is_carried_when_the_caller_knows_it() {
    let request = policy_request_for("Read", &json!({}), Some("claude-opus-5"));
    assert_eq!(request.model.as_deref(), Some("claude-opus-5"));
    assert_eq!(request.risk, Some(RiskTier::Low));
}
