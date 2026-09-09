use super::*;
use crate::agent::multi::role::SpecializedRole;

#[test]
fn researcher_denied_tools_filtered() {
    let role = SpecializedRole::researcher();
    let base_tools = vec![
        "Read".into(),
        "Edit".into(),
        "Write".into(),
        "Bash".into(),
        "mcp__workbench-mcp__search".into(),
    ];
    let filtered = apply_tool_policy(&role, &base_tools);
    assert!(filtered.contains(&"Read".to_string()));
    assert!(filtered.contains(&"Bash".to_string()));
    assert!(!filtered.contains(&"Edit".to_string()));
    assert!(!filtered.contains(&"Write".to_string()));
}

#[test]
fn coder_keeps_all_tools() {
    let role = SpecializedRole::coder();
    let base_tools = vec!["Read".into(), "Edit".into(), "Write".into(), "Bash".into()];
    let filtered = apply_tool_policy(&role, &base_tools);
    assert_eq!(filtered.len(), 4);
}

#[test]
fn allowlist_restricts_to_subset() {
    let mut role = SpecializedRole::coder();
    role.allowed_tools = vec!["Read".into(), "Bash".into()];
    role.denied_tools = vec![];
    let base_tools = vec!["Read".into(), "Edit".into(), "Write".into(), "Bash".into()];
    let filtered = apply_tool_policy(&role, &base_tools);
    assert_eq!(filtered.len(), 2);
    assert!(filtered.contains(&"Read".to_string()));
    assert!(filtered.contains(&"Bash".to_string()));
}

#[test]
fn deny_takes_precedence_over_allow() {
    let mut role = SpecializedRole::coder();
    role.allowed_tools = vec!["Read".into(), "Edit".into()];
    role.denied_tools = vec!["Edit".into()];
    let base_tools = vec!["Read".into(), "Edit".into(), "Bash".into()];
    let filtered = apply_tool_policy(&role, &base_tools);
    assert_eq!(filtered, vec!["Read".to_string()]);
}

#[test]
fn empty_base_tools_returns_empty() {
    let role = SpecializedRole::researcher();
    let filtered = apply_tool_policy(&role, &[]);
    assert!(filtered.is_empty());
}

#[test]
fn is_tool_allowed_checks_policy() {
    let role = SpecializedRole::researcher();
    assert!(!is_tool_allowed(&role, "Edit"));
    assert!(!is_tool_allowed(&role, "Write"));
    assert!(is_tool_allowed(&role, "Read"));
    assert!(is_tool_allowed(&role, "Bash"));
}

#[test]
fn is_tool_allowed_with_allowlist() {
    let mut role = SpecializedRole::coder();
    role.allowed_tools = vec!["Read".into()];
    assert!(is_tool_allowed(&role, "Read"));
    assert!(!is_tool_allowed(&role, "Edit"));
}
