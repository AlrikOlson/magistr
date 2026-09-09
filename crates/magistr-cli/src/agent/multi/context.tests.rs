use super::*;
use crate::agent::multi::topology::AgentId;

#[test]
fn empty_context() {
    let ctx = SharedContext::new();
    assert_eq!(ctx.total_findings(), 0);
    assert!(ctx.keys().is_empty());
    assert!(ctx.query("anything").is_empty());
}

#[test]
fn publish_and_query() {
    let ctx = SharedContext::new();
    let agent = AgentId::new(1);

    ctx.publish(agent, "api_pattern", "REST with pagination");
    ctx.publish(agent, "api_pattern", "OAuth2 for auth");
    ctx.publish(AgentId::new(2), "test_strategy", "use mocks");

    let api_findings = ctx.query("api_pattern");
    assert_eq!(api_findings.len(), 2);
    assert_eq!(api_findings[0].value, "REST with pagination");
    assert_eq!(api_findings[1].value, "OAuth2 for auth");

    let test_findings = ctx.query("test_strategy");
    assert_eq!(test_findings.len(), 1);
    assert_eq!(test_findings[0].value, "use mocks");
}

#[test]
fn findings_by_agent() {
    let ctx = SharedContext::new();
    let a1 = AgentId::new(1);
    let a2 = AgentId::new(2);

    ctx.publish(a1, "key1", "val1");
    ctx.publish(a1, "key2", "val2");
    ctx.publish(a2, "key1", "val3");

    let a1_findings = ctx.findings_by_agent(a1);
    assert_eq!(a1_findings.len(), 2);

    let a2_findings = ctx.findings_by_agent(a2);
    assert_eq!(a2_findings.len(), 1);
    assert_eq!(a2_findings[0].value, "val3");
}

#[test]
fn keys_lists_all_keys() {
    let ctx = SharedContext::new();
    ctx.publish(AgentId::new(1), "alpha", "a");
    ctx.publish(AgentId::new(1), "beta", "b");
    ctx.publish(AgentId::new(2), "alpha", "c"); // duplicate key

    let mut keys = ctx.keys();
    keys.sort();
    assert_eq!(keys, vec!["alpha", "beta"]);
}

#[test]
fn total_findings_counts_all() {
    let ctx = SharedContext::new();
    ctx.publish(AgentId::new(1), "k", "v1");
    ctx.publish(AgentId::new(1), "k", "v2");
    ctx.publish(AgentId::new(2), "j", "v3");
    assert_eq!(ctx.total_findings(), 3);
}

#[test]
fn clear_removes_everything() {
    let ctx = SharedContext::new();
    ctx.publish(AgentId::new(1), "k", "v");
    assert_eq!(ctx.total_findings(), 1);

    ctx.clear();
    assert_eq!(ctx.total_findings(), 0);
    assert!(ctx.keys().is_empty());
}

#[test]
fn clone_shares_state() {
    let ctx = SharedContext::new();
    let ctx2 = ctx.clone();

    ctx.publish(AgentId::new(1), "shared", "data");
    assert_eq!(ctx2.total_findings(), 1);
    assert_eq!(ctx2.query("shared")[0].value, "data");
}

// ─── build_injection ─────────────────────────────────────────────────────

#[test]
fn build_injection_empty_context() {
    let ctx = SharedContext::new();
    assert!(ctx.build_injection("coder", "some task").is_empty());
}

#[test]
fn build_injection_coder_gets_research_findings() {
    let ctx = SharedContext::new();
    ctx.publish(AgentId::new(1), "research_findings", "Found REST API");
    ctx.publish(AgentId::new(1), "unrelated_key", "noise");

    let injection = ctx.build_injection("coder", "implement auth");
    assert!(injection.contains("Found REST API"));
    assert!(!injection.contains("noise"));
}

#[test]
fn build_injection_researcher_gets_everything() {
    let ctx = SharedContext::new();
    ctx.publish(AgentId::new(1), "research_findings", "A");
    ctx.publish(AgentId::new(2), "unrelated_key", "B");

    let injection = ctx.build_injection("researcher", "explore");
    assert!(injection.contains("A"));
    assert!(injection.contains("B"));
}

#[test]
fn build_injection_reviewer_gets_coder_output() {
    let ctx = SharedContext::new();
    ctx.publish(AgentId::new(1), "coder_output", "implemented X");
    ctx.publish(AgentId::new(2), "test_results", "all pass");

    let injection = ctx.build_injection("reviewer", "review code");
    assert!(injection.contains("implemented X"));
    assert!(!injection.contains("all pass"));
}

// ─── extract_and_publish ─────────────────────────────────────────────────

#[test]
fn extract_and_publish_uses_role_key() {
    let ctx = SharedContext::new();
    let agent = AgentId::new(1);

    ctx.extract_and_publish(agent, "researcher", "Found pattern X");
    let findings = ctx.query("research_findings");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].value, "Found pattern X");
}

#[test]
fn extract_and_publish_coder_output() {
    let ctx = SharedContext::new();
    ctx.extract_and_publish(AgentId::new(2), "coder", "Added login endpoint");
    assert_eq!(ctx.query("coder_output").len(), 1);
}

#[test]
fn extract_and_publish_unknown_role_uses_agent_output() {
    let ctx = SharedContext::new();
    ctx.extract_and_publish(AgentId::new(3), "custom_role", "did stuff");
    assert_eq!(ctx.query("agent_output").len(), 1);
}
