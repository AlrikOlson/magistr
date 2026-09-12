//! Portable scaffold commands exercised as subprocesses: the generated
//! `magistr mcp-launch workbench-mcp .` line serves this binary's own native
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
    scaffold::generate(root, &test_vars(), &HashMap::new()).expect("generate project");
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
        .env("HOME", root)
        .env("PATH", "")
        .env("WORKBENCH_NETWORK_FS", "0")
        .env_remove("WORKBENCH_ORG_POLICY")
        .env_remove("WORKBENCH_USER_CONFIG")
        .env_remove("WORKBENCH_MCP")
        .env_remove("WORKBENCH_MCP_REPO")
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
        expected.join(".workbench/server.generation").is_file(),
        "no native admission under {}",
        expected.display()
    );
    for other in others {
        assert!(
            !other.join(".workbench").exists(),
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
    let preview = scaffold::preview(&root, &test_vars(), &HashMap::new()).expect("preview");
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
    for target in ["magistr", "workbench-mcp"] {
        assert_eq!(
            config["mcpServers"][target],
            json!({"type": "stdio", "command": "magistr", "args": ["mcp-launch", target, "."]})
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
    let entry = &config["mcpServers"]["workbench-mcp"];
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
fn nearest_legacy_project_is_selected_from_a_nested_directory() {
    let temporary = tempfile::tempdir().expect("temporary projects");
    generate_project(temporary.path());
    let inner = temporary.path().join("nested project");
    std::fs::create_dir_all(inner.join(".jig")).expect("legacy project directory");
    std::fs::write(inner.join(".jig/project.json"), "{}").expect("legacy project marker");
    let nested = inner.join("src/deeper");
    std::fs::create_dir_all(&nested).expect("nested directory");
    let mut command = fixture_command(&nested);
    command.args(["mcp-launch", "workbench-mcp"]);
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
    std::fs::write(repository.join(".git"), "gitdir: ignored-by-fixture\n")
        .expect("worktree boundary");
    let mut command = fixture_command(temporary.path());
    command.args(["mcp-launch", "workbench-mcp"]).arg(&nested);
    invoke_native(command);
    assert_workspace(&nested, &[temporary.path(), &repository]);
}

#[test]
fn legacy_resolver_overrides_are_ignored_and_nothing_external_is_required() {
    // The retired resolver read WORKBENCH_MCP and WORKBENCH_MCP_REPO and fell
    // back to PATH or a home checkout. The native launch consults none of
    // them: bogus values change nothing and an empty PATH is enough.
    let temporary = tempfile::tempdir().expect("temporary project");
    let original_config = generate_project(temporary.path());
    let missing = temporary.path().join("missing executable λ");
    let mut command = fixture_command(temporary.path());
    command
        .args(["mcp-launch", "workbench-mcp", "."])
        .env("WORKBENCH_MCP", &missing)
        .env("WORKBENCH_MCP_REPO", temporary.path().join("no checkout"));
    let output = invoke_native(command);
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains("missing executable λ"),
        "the override was consulted"
    );
    assert_eq!(read_config(temporary.path()), original_config);
    assert_workspace(temporary.path(), &[]);
}
