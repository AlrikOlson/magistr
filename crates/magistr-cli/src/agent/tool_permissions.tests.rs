use super::*;

#[test]
fn base_includes_web_search() {
    let perms = ToolPermissions::base();
    assert!(perms.contains("web_search"), "base must include web_search");
}

#[test]
fn interview_includes_codebase_and_bash() {
    let perms = ToolPermissions::for_interview();
    for tool in &["Read", "Glob", "Grep", "Bash", "web_search"] {
        assert!(perms.contains(tool), "interview must include {tool}");
    }
}

#[test]
fn plan_includes_codebase_read_only() {
    let perms = ToolPermissions::for_plan();
    for tool in &["Read", "Glob", "Grep", "web_search"] {
        assert!(perms.contains(tool), "plan must include {tool}");
    }
    assert!(!perms.contains("Bash"), "plan must NOT include Bash");
    assert!(
        perms.contains("execute"),
        "plan generation authors tasks through plan_add_task"
    );
}

#[test]
fn no_permission_set_names_the_retired_plan_tools() {
    for perms in [
        ToolPermissions::base(),
        ToolPermissions::for_interview(),
        ToolPermissions::for_plan(),
        ToolPermissions::for_workflow(),
    ] {
        assert!(
            !perms
                .as_slice()
                .iter()
                .any(|t| t.starts_with("magistr_plan")),
            "the retired magistr plan tools gave way to plan_*"
        );
    }
}

#[test]
fn workflow_includes_essentials() {
    let perms = ToolPermissions::for_workflow();
    for tool in &["Read", "Bash", "Glob", "Grep", "web_search"] {
        assert!(perms.contains(tool), "workflow must include {tool}");
    }
    // Write/Edit intentionally excluded — routed through permission handler
    assert!(!perms.contains("Write"), "Write must NOT be pre-approved");
    assert!(!perms.contains("Edit"), "Edit must NOT be pre-approved");
}

#[test]
fn workflow_is_superset_of_base() {
    let base = ToolPermissions::base();
    let workflow = ToolPermissions::for_workflow();
    for tool in base.as_slice() {
        assert!(
            workflow.contains(tool),
            "workflow must include base tool {tool}"
        );
    }
}

#[test]
fn workflow_includes_mcp_tools() {
    let perms = ToolPermissions::for_workflow();
    for tool in UNIFIED_MCP {
        assert!(
            perms.contains(tool),
            "workflow must include MCP tool {tool}"
        );
    }
}

#[test]
fn reasoning_lifecycle_is_available_to_workflow_and_plan_agents() {
    for permissions in [
        ToolPermissions::for_workflow(),
        ToolPermissions::for_plan(),
        ToolPermissions::for_plan_request(),
    ] {
        assert!(permissions.contains("discover"));
        assert!(permissions.contains("execute"));
        assert!(!permissions.contains("think_wipe_trace"));
        assert!(!permissions.contains("think_export_trace"));
    }
    assert!(!ToolPermissions::for_interview().contains("think_set_branch_status"));
}

#[test]
fn with_deduplicates() {
    let perms = ToolPermissions::base()
        .with(&["web_search", "extra"])
        .with(&["extra"]);
    let count = perms.as_slice().iter().filter(|t| **t == "extra").count();
    assert_eq!(count, 1, "with() must deduplicate");
}

#[test]
fn into_strings_converts() {
    let strings = ToolPermissions::base().into_strings();
    assert_eq!(strings, vec!["web_search".to_string()]);
}
