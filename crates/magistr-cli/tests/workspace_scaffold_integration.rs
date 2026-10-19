//! A freshly scaffolded project declares the unified server and native commands.

use std::collections::HashMap;
use std::process::Command;

fn template_vars() -> HashMap<String, String> {
    [
        ("project_name", "wbfix"),
        ("project_description", "workspace fixture"),
        ("project_type", "cli"),
        ("language", "rust"),
        ("framework", "none"),
        ("package_manager", "cargo"),
        ("test_framework", "cargo-test"),
        ("linter", "clippy"),
        ("formatter", "rustfmt"),
        ("check_cmd", "cargo check --all-targets"),
        ("test_cmd", "cargo test"),
        ("lint_cmd", "cargo clippy -- -D warnings"),
        ("format_cmd", "cargo fmt"),
        ("format_extensions", r#""rs""#),
        ("companion_pattern", "tests/{name}.rs"),
        ("test_dir", "tests/"),
        ("state_prefix", "wbfix"),
        ("allowed_tools", ""),
        ("mcp_permissions", r#""mcp__magistr""#),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

#[test]
fn scaffolded_fixture_is_a_workspace_workspace() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    std::fs::create_dir_all(dir.join("src")).expect("src");
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"wbfix\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("Cargo.toml");
    std::fs::write(dir.join("src/main.rs"), "fn main() {}\n").expect("main.rs");

    magistr::scaffold::generate(
        dir,
        &template_vars(),
        &HashMap::new(),
        &magistr::scaffold::SetupSeeds::default(),
    )
    .expect("scaffold");

    // One registration supplies workflow and source capabilities.
    let mcp: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join(".mcp.json")).expect("mcp"))
            .expect("json");
    let servers = mcp["mcpServers"].as_object().expect("servers");
    assert_eq!(servers.len(), 1);
    assert!(servers.contains_key("magistr"));
    assert!(!servers.contains_key("magistr-scope"));
    let args = servers["magistr"]["args"].as_array().expect("args");
    // The workspace root is the last argument — "." on purpose: a committed
    // .mcp.json carries no path from the machine that ran init.
    assert_eq!(
        args.last().and_then(|v| v.as_str()),
        Some("."),
        "workspace root is the last argument, written portably"
    );

    // The routing skill and the generated tool guide point at workspace.
    let skill = std::fs::read_to_string(dir.join(".claude/skills/workspace-routing/SKILL.md"))
        .expect("skill");
    assert!(skill.contains("name: workspace-routing"));
    let tools = std::fs::read_to_string(dir.join(".claude/rules/tools.md")).expect("tools.md");
    assert!(tools.contains("discover"));
    assert!(!tools.contains("scope_search"));

    // The gates are declared for workspace `run`.
    let toml_body = std::fs::read_to_string(dir.join(".magistr/workspace.toml"))
        .expect(".magistr/workspace.toml");
    let parsed: toml::Value = toml::from_str(&toml_body).expect("valid toml");
    for key in ["check", "test", "lint", "format"] {
        assert!(
            parsed["commands"].get(key).is_some(),
            "{key} declared in [commands]"
        );
    }

    assert!(
        parsed["commands"].get("cargo-test").is_some(),
        "init-seeded cargo-test present: {toml_body}"
    );
    let status = Command::new(env!("CARGO_BIN_EXE_magistr"))
        .args(["workspace", "doctor"])
        .arg(dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("run doctor");
    assert!(status.success(), "workspace doctor reported findings");
}
