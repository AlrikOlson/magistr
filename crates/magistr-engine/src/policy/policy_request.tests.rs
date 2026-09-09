use super::*;

#[test]
fn a_fresh_request_knows_nothing() {
    let request = PolicyRequest::new();
    assert!(request.tool.is_none());
    assert!(request.paths.is_empty());
    assert_eq!(request.summary(), "");
}

#[test]
fn the_builder_fills_each_field() {
    let request = PolicyRequest::tool("Bash")
        .with_command("rm -rf /")
        .with_path("/etc/passwd")
        .with_model("claude-opus-5")
        .with_risk(RiskTier::High)
        .with_gate("test");
    assert_eq!(request.tool.as_deref(), Some("Bash"));
    assert_eq!(request.command.as_deref(), Some("rm -rf /"));
    assert_eq!(request.paths, vec!["/etc/passwd".to_string()]);
    assert_eq!(request.model.as_deref(), Some("claude-opus-5"));
    assert_eq!(request.risk, Some(RiskTier::High));
    assert_eq!(request.gate.as_deref(), Some("test"));
}

#[test]
fn summary_names_every_populated_field() {
    let summary = PolicyRequest::tool("Write")
        .with_path("src/main.rs")
        .with_risk(RiskTier::Medium)
        .summary();
    assert_eq!(summary, "tool=Write paths=src/main.rs risk=medium");
}
