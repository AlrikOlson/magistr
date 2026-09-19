//! A freshly scaffolded project is a healthy workbench workspace.
//!
//! Runs only where `workbench-mcp` is on PATH; elsewhere the test passes
//! trivially after checking the scaffold output alone.

use std::collections::HashMap;
use std::process::Command;

fn template_vars() -> HashMap<String, String> {
    [
        ("project_name", "wbfix"),
        ("project_description", "workbench fixture"),
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
        ("mcp_permissions", r#""mcp__magistr", "mcp__workbench-mcp""#),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

#[test]
fn scaffolded_fixture_is_a_workbench_workspace() {
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

    // The navigation substrate is workbench-mcp, and only workbench-mcp.
    let mcp: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join(".mcp.json")).expect("mcp"))
            .expect("json");
    let servers = mcp["mcpServers"].as_object().expect("servers");
    assert!(servers.contains_key("workbench-mcp"));
    assert!(!servers.contains_key("magistr-scope"));
    let args = servers["workbench-mcp"]["args"].as_array().expect("args");
    // The workspace root is the last argument — "." on purpose: a committed
    // .mcp.json carries no path from the machine that ran init.
    assert_eq!(
        args.last().and_then(|v| v.as_str()),
        Some("."),
        "workspace root is the last argument, written portably"
    );

    // The routing skill and the generated tool guide point at workbench.
    let skill = std::fs::read_to_string(dir.join(".claude/skills/workbench-routing/SKILL.md"))
        .expect("skill");
    assert!(skill.contains("name: workbench-routing"));
    let tools = std::fs::read_to_string(dir.join(".claude/rules/tools.md")).expect("tools.md");
    assert!(tools.contains("workbench-mcp"));
    assert!(!tools.contains("scope_search"));

    // The gates are declared for workbench `run`.
    let toml_body = std::fs::read_to_string(dir.join("workbench.toml")).expect("workbench.toml");
    let parsed: toml::Value = toml::from_str(&toml_body).expect("valid toml");
    for key in ["check", "test", "lint", "format"] {
        assert!(
            parsed["commands"].get(key).is_some(),
            "{key} declared in [commands]"
        );
    }

    let Ok(doctor) = which::which("workbench-mcp") else {
        eprintln!("workbench-mcp not on PATH; skipping the doctor probe");
        return;
    };
    // `workbench-mcp init` ran during the scaffold, so manifest-detected
    // commands sit beside the gate keys.
    assert!(
        parsed["commands"].get("cargo-test").is_some(),
        "init-seeded cargo-test present: {toml_body}"
    );
    let status = Command::new(doctor)
        .arg("doctor")
        .arg(dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("run doctor");
    assert!(status.success(), "workbench-mcp doctor reported findings");
}
