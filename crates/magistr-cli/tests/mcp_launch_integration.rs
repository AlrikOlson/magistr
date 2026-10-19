//! Portable scaffold commands exercised as subprocesses: the generated
//! `magistr mcp --dir .` line serves this binary's own native
//! workspace bridge for the project it selects, with nothing on PATH and no
//! environment override consulted.
#![cfg(unix)]

#[path = "support/magistr_command.rs"]
mod magistr_command;

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use magistr::scaffold;
use serde_json::{Value, json};

fn test_vars() -> HashMap<String, String> {
    [
        ("project_name", "portable-fixture"),
        ("project_description", "Portable MCP fixture"),
        ("language", "rust"),
        ("framework", "none"),
        ("package_manager", "cargo"),
        ("test_framework", "cargo-test"),
        ("linter", "clippy"),
        ("formatter", "rustfmt"),
        ("check_cmd", "cargo check"),
        ("test_cmd", "cargo test"),
        ("lint_cmd", "cargo clippy -- -D warnings"),
        ("format_cmd", "cargo fmt"),
        ("format_extensions", r#""rs""#),
        ("companion_pattern", "tests/{name}.rs"),
        ("test_dir", "tests/"),
        ("state_prefix", "portable-fixture"),
        ("reference_repos", ""),
        ("allowed_tools", r#""Bash(cargo:*)""#),
        ("project_type", "cli"),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_owned(), value.to_owned()))
    .collect()
}

fn generate_project(root: &Path) -> Value {
    std::fs::create_dir_all(root).expect("project directory");
    scaffold::generate(
        root,
        &test_vars(),
        &HashMap::new(),
        &scaffold::SetupSeeds::default(),
    )
    .expect("generate project");
    read_config(root)
}

fn read_config(root: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(root.join(".mcp.json")).expect("MCP config"))
        .expect("MCP JSON")
}

fn fixture_command(root: &Path) -> Command {
    let mut command = magistr_command::magistr_command();
    command
        .current_dir(root)
        .env("MAGISTR_WORKFLOW_DATA_DIR", root.join("state"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env("PATH", "")
        .env("MAGISTR_WORKSPACE_NETWORK_FS", "0")
        .env_remove("MAGISTR_WORKSPACE_ORG_POLICY")
        .env_remove("MAGISTR_WORKSPACE_USER_CONFIG")
        .env_remove("MAGISTR_WORKSPACE_MCP")
        .env_remove("MAGISTR_WORKSPACE_MCP_REPO")
        .env_remove("MAGISTR_LOG")
        .env_remove("MAGISTR_LOG_FILE")
        .env("NO_COLOR", "1");
    command
}

/// Drive one MCP initialization through the launched bridge's stdio and let
/// EOF end it. The answer is this binary's own server, and the process exits
/// cleanly once its client leaves.
fn invoke_native(mut command: Command) -> Output {
    const INPUT: &[u8] = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{},\"clientInfo\":{\"name\":\"fixture\",\"version\":\"0\"}}}\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n";
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("start portable command");
    child
        .stdin
        .take()
        .expect("piped input")
        .write_all(INPUT)
        .expect("write fixture input");
    let output = child.wait_with_output().expect("portable command output");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let answer: Value = serde_json::from_str(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .expect("an initialize answer"),
    )
    .expect("JSON-RPC answer");
    assert_eq!(answer["id"], 1, "{answer}");
    assert_eq!(
        answer["result"]["serverInfo"]["name"], "magistr",
        "{answer}"
    );
    output
}

/// The native owner admitted exactly `expected`: its state directory carries
/// the durable generation, and no other candidate root gained one.
fn assert_workspace(expected: &Path, others: &[&Path]) {
    assert!(
        expected
            .join(".magistr/workspace/server.generation")
            .is_file(),
        "no native admission under {}",
        expected.display()
    );
    for other in others {
        assert!(
            !other.join(".magistr/workspace").exists(),
            "an unrelated root was admitted: {}",
            other.display()
        );
    }
}

#[test]
fn generated_servers_are_portable_and_match_the_preview() {
    let temporary = tempfile::tempdir().expect("temporary project");
    let root = temporary.path().join("project with spaces λ");
    std::fs::create_dir(&root).expect("project directory");
    let custom = json!({
        "command": "custom-server",
        "args": ["literal argument"],
        "env": {"CUSTOM_SETTING": "preserve me"}
    });
    std::fs::write(
        root.join(".mcp.json"),
        serde_json::to_vec(&json!({"mcpServers": {"custom": custom}})).expect("custom JSON"),
    )
    .expect("custom server");
    let preview = scaffold::preview(
        &root,
        &test_vars(),
        &HashMap::new(),
        &scaffold::SetupSeeds::default(),
    )
    .expect("preview");
    let proposed = &preview
        .files
        .iter()
        .find(|file| file.path == ".mcp.json")
        .expect("MCP preview")
        .new_content;
    let config = generate_project(&root);
    assert_eq!(
        config,
        serde_json::from_str::<Value>(proposed).expect("preview JSON")
    );
    {
        let target = "magistr";
        assert_eq!(
            config["mcpServers"][target],
            json!({"type": "stdio", "command": "magistr", "args": ["mcp", "--dir", "."]})
        );
    }
    assert_eq!(config["mcpServers"]["custom"], custom);
    assert_eq!(
        generate_project(&root),
        config,
        "regeneration preserves the merged configuration"
    );
    assert!(!proposed.contains(&root.to_string_lossy().to_string()));
}

#[test]
fn relocated_project_is_served_natively_from_a_nested_directory() {
    let temporary = tempfile::tempdir().expect("temporary relocation");
    let original = temporary.path().join("old clone λ");
    let config = generate_project(&original);
    let relocated = temporary.path().join("new clone with spaces λ");
    std::fs::rename(&original, &relocated).expect("relocate project");
    let nested = relocated.join("src/nested");
    std::fs::create_dir_all(&nested).expect("nested working directory");
    let entry = &config["mcpServers"]["magistr"];
    assert_eq!(entry["command"], "magistr");
    let mut command = fixture_command(&nested);
    command.args(
        entry["args"]
            .as_array()
            .expect("generated argv")
            .iter()
            .map(|argument| argument.as_str().expect("string argument")),
    );
    invoke_native(command);
    assert_workspace(&relocated, &[&nested]);
    assert_eq!(
        read_config(&relocated),
        config,
        "launch leaves the portable config intact"
    );
}

#[test]
fn nearest_configured_project_is_selected_from_a_nested_directory() {
    let temporary = tempfile::tempdir().expect("temporary projects");
    generate_project(temporary.path());
    let inner = temporary.path().join("nested project");
    std::fs::create_dir_all(inner.join(".magistr")).expect("legacy project directory");
    std::fs::write(inner.join(".magistr/project.json"), "{}").expect("legacy project marker");
    let nested = inner.join("src/deeper");
    std::fs::create_dir_all(&nested).expect("nested directory");
    let mut command = fixture_command(&nested);
    command.args(["mcp"]);
    invoke_native(command);
    assert_workspace(&inner, &[temporary.path(), &nested]);
}

#[test]
fn repository_boundary_keeps_an_unconfigured_project_out_of_its_parent() {
    let temporary = tempfile::tempdir().expect("temporary projects");
    generate_project(temporary.path());
    let repository = temporary.path().join("separate worktree");
    let nested = repository.join("src/deeper");
    std::fs::create_dir_all(&nested).expect("nested directory");
    std::fs::create_dir(repository.join(".git")).expect("worktree boundary");
    let mut command = fixture_command(temporary.path());
    command.args(["mcp", "--dir"]).arg(&nested);
    invoke_native(command);
    assert_workspace(&repository, &[temporary.path(), &nested]);
}

#[test]
fn launch_needs_no_external_executable_or_checkout() {
    let temporary = tempfile::tempdir().expect("temporary project");
    let original = generate_project(temporary.path());
    let mut command = fixture_command(temporary.path());
    command.args(["mcp", "--dir", "."]);
    invoke_native(command);
    assert_eq!(read_config(temporary.path()), original);
    assert_workspace(temporary.path(), &[]);
}
