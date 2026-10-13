use super::*;

#[test]
fn incomplete_shell_input_has_unknown_risk() {
    for input in [
        serde_json::Value::Null,
        serde_json::json!({}),
        serde_json::json!("git status"),
        serde_json::json!({"command": null}),
        serde_json::json!({"command": false}),
        serde_json::json!({"command": 42}),
        serde_json::json!({"command": ["git", "status"]}),
        serde_json::json!({"command": {"command": "git status"}}),
        serde_json::json!({"command": ""}),
        serde_json::json!({"command": " \t\r\n"}),
        serde_json::json!({"command": "\u{2003}\u{00a0}"}),
    ] {
        assert_eq!(classify_risk("Bash", &input), RiskLevel::High, "{input}");
    }
}
