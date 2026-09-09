//! Portable scaffold commands exercised as subprocesses without installed MCP servers.
//! The process fixtures use a POSIX shell so exact stdio and exit behavior are observable.
#![cfg(unix)]

#[path = "support/magistr_command.rs"]
mod magistr_command;

use std::collections::HashMap;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
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
        .env_remove("WORKBENCH_MCP")
        .env_remove("WORKBENCH_MCP_REPO")
        .env_remove("MAGISTR_LOG")
        .env_remove("MAGISTR_LOG_FILE")
        .env("NO_COLOR", "1");
    command
}

fn write_dependency(root: &Path) -> PathBuf {
    let executable = root.join("workbench fixture λ");
    std::fs::write(
        &executable,
        r#"#!/bin/sh
printf '%s\n' "$PWD" > "$LAUNCH_CWD"
printf '%s\n' "$#" "$@" > "$LAUNCH_ARGS"
printf 'dependency diagnostic\n' >&2
while IFS= read -r line; do
    printf '%s\n' "$line"
done
exit 23
"#,
    )
    .expect("dependency fixture");
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755))
        .expect("executable fixture");
    executable
}

fn invoke_dependency(mut command: Command, executable: &Path, receipts: &Path) -> Output {
    command.env("WORKBENCH_MCP", executable);
    invoke_fixture(command, receipts)
}

fn invoke_fixture(mut command: Command, receipts: &Path) -> Output {
    const INPUT: &[u8] =
        b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\nbackslash \\ and literal $(text)\n";
    command
        .env("LAUNCH_CWD", receipts.join("cwd"))
        .env("LAUNCH_ARGS", receipts.join("args"))
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
        Some(23),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, INPUT);
    assert_eq!(output.stderr, b"dependency diagnostic\n");
    output
}

fn assert_workspace(receipts: &Path, expected: &Path) {
    let expected = expected.canonicalize().expect("canonical workspace");
    assert_eq!(
        std::fs::read_to_string(receipts.join("cwd")).expect("cwd receipt"),
        format!("{}\n", expected.display())
    );
    assert_eq!(
        std::fs::read_to_string(receipts.join("args")).expect("argv receipt"),
        format!("1\n{}\n", expected.display())
    );
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
fn relocated_project_uses_runtime_dependency_and_preserves_process_io() {
    let temporary = tempfile::tempdir().expect("temporary relocation");
    let original = temporary.path().join("old clone λ");
    let config = generate_project(&original);
    let relocated = temporary.path().join("new clone with spaces λ");
    std::fs::rename(&original, &relocated).expect("relocate project");
    let nested = relocated.join("src/nested");
    std::fs::create_dir_all(&nested).expect("nested working directory");
    let dependency = write_dependency(temporary.path());
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
    invoke_dependency(command, &dependency, temporary.path());
    assert_workspace(temporary.path(), &relocated);
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
    let dependency = write_dependency(temporary.path());
    let mut command = fixture_command(&nested);
    command.args(["mcp-launch", "workbench-mcp"]);
    invoke_dependency(command, &dependency, temporary.path());
    assert_workspace(temporary.path(), &inner);
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
    let dependency = write_dependency(temporary.path());
    let mut command = fixture_command(temporary.path());
    command.args(["mcp-launch", "workbench-mcp"]).arg(&nested);
    invoke_dependency(command, &dependency, temporary.path());
    assert_workspace(temporary.path(), &nested);
}

#[test]
fn unavailable_dependencies_fail_with_an_actionable_stderr_message() {
    let temporary = tempfile::tempdir().expect("temporary missing dependency");
    let original_config = generate_project(temporary.path());
    let missing = temporary.path().join("missing executable λ");
    for explicit_override in [false, true] {
        let mut command = fixture_command(temporary.path());
        command.args(["mcp-launch", "workbench-mcp", "."]);
        if explicit_override {
            command.env("WORKBENCH_MCP", &missing);
        }
        let output = command.output().expect("run missing dependency case");
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty(), "diagnostic leaked to MCP stdout");
        assert!(
            diagnostic.contains("WORKBENCH_MCP") || diagnostic.to_lowercase().contains("install"),
            "missing remediation: {diagnostic}"
        );
        if explicit_override {
            assert!(diagnostic.contains("missing executable λ"), "{diagnostic}");
        }
        assert_eq!(read_config(temporary.path()), original_config);
    }
}

#[test]
fn relative_executable_override_is_resolved_before_selecting_the_workspace() {
    let temporary = tempfile::tempdir().expect("temporary relative executable");
    generate_project(temporary.path());
    let nested = temporary.path().join("src/nested");
    std::fs::create_dir_all(&nested).expect("nested directory");
    let executable = write_dependency(&nested);
    let relative = Path::new(".").join(executable.file_name().expect("fixture filename"));
    let mut command = fixture_command(&nested);
    command.args(["mcp-launch", "workbench-mcp"]);
    invoke_dependency(command, &relative, temporary.path());
    assert_workspace(temporary.path(), temporary.path());
}

#[test]
fn relative_path_entry_keeps_the_dependency_found_from_the_invocation_directory() {
    let temporary = tempfile::tempdir().expect("temporary relative PATH");
    generate_project(temporary.path());
    let nested = temporary.path().join("src/nested");
    let tools = nested.join("tools λ");
    std::fs::create_dir_all(&tools).expect("fixture tools directory");
    let executable = write_dependency(&tools);
    std::fs::rename(executable, tools.join("workbench-mcp")).expect("installed Workbench fixture");
    let mut command = fixture_command(&nested);
    command
        .args(["mcp-launch", "workbench-mcp"])
        .env("PATH", "tools λ");
    invoke_fixture(command, temporary.path());
    assert_workspace(temporary.path(), temporary.path());
}

#[test]
fn relative_checkout_passes_an_absolute_entrypoint_and_workspace_to_bun() {
    let temporary = tempfile::tempdir().expect("temporary relative checkout");
    generate_project(temporary.path());
    let nested = temporary.path().join("src/nested");
    let checkout = nested.join("checkout λ");
    let tools = temporary.path().join("fixture tools");
    std::fs::create_dir_all(checkout.join("src")).expect("checkout directory");
    std::fs::create_dir(&tools).expect("Bun fixture directory");
    let main = checkout.join("src/main.ts");
    std::fs::write(
        &main,
        "// The Bun fixture records this path without running it.\n",
    )
    .expect("checkout entrypoint");
    let executable = write_dependency(&tools);
    std::fs::rename(executable, tools.join("bun")).expect("Bun fixture");
    let mut command = fixture_command(&nested);
    command
        .args(["mcp-launch", "workbench-mcp"])
        .env("PATH", &tools)
        .env("WORKBENCH_MCP_REPO", "checkout λ");
    invoke_fixture(command, temporary.path());
    let root = temporary.path().canonicalize().expect("canonical project");
    assert_eq!(
        std::fs::read_to_string(temporary.path().join("cwd")).expect("cwd receipt"),
        format!("{}\n", root.display())
    );
    assert_eq!(
        std::fs::read_to_string(temporary.path().join("args")).expect("argv receipt"),
        format!(
            "3\nrun\n{}\n{}\n",
            main.canonicalize().expect("canonical entrypoint").display(),
            root.display()
        )
    );
}
