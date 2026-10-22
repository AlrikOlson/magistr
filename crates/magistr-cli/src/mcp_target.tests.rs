use super::*;

fn npx_entry(package: &str) -> serde_json::Value {
    serde_json::json!({
        "command": "npx",
        "args": ["--yes", "--package", package, "magistr", "mcp", "--dir", "."],
        "type": "stdio"
    })
}

#[test]
fn generated_npx_entries_are_owned_for_every_semantic_version() {
    let root = std::path::Path::new(".");
    for package in [
        "magistr@0.1.0",
        "magistr@1.0.0-beta.1",
        "magistr@1.0.0-rc.2",
        "magistr@2.3.4-alpha.1+build.7",
        "magistr@10.20.30",
    ] {
        assert!(
            McpTarget.owns_entry(root, &npx_entry(package)),
            "{package} is a generated pinned registration"
        );
    }
}

#[test]
fn unpinned_or_malformed_npx_entries_are_customized_launchers() {
    let root = std::path::Path::new(".");
    for package in [
        "magistr",
        "magistr@latest",
        "magistr@beta",
        "magistr@1.0",
        "magistr@1.0.0-",
        "magistr@01.0.0",
        "magistr@1.0.0-beta..1",
        "other@1.0.0",
    ] {
        assert!(
            !McpTarget.owns_entry(root, &npx_entry(package)),
            "{package} must be treated as a customized launcher"
        );
    }
}

#[test]
fn generated_direct_entries_are_owned_only_for_this_project() {
    let root = std::path::Path::new(".");
    let owned =
        serde_json::json!({"command": "magistr", "args": ["mcp", "--dir", "."], "type": "stdio"});
    assert!(McpTarget.owns_entry(root, &owned));
    let elsewhere = serde_json::json!({"command": "magistr", "args": ["mcp", "--dir", "/somewhere/else"], "type": "stdio"});
    assert!(!McpTarget.owns_entry(root, &elsewhere));
    let wrapped = serde_json::json!({"command": "magistr", "args": ["mcp", "--dir", "."], "type": "stdio", "env": {}});
    assert!(!McpTarget.owns_entry(root, &wrapped));
}
