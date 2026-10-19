use super::format::{build_find_format_command, resolve_format_command};
use super::gate::run_gate_command;
use super::*;
use magistr_core::FormatConfig;
use serde_json::json;
use tempfile::TempDir;

fn test_root() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    (tmp, root)
}

#[test]
fn workflow_file_tools_are_absent() {
    let (_tmp, root) = test_root();
    let server = build_server(root);
    assert!(server.get_tool("magistr_delete").is_none());
    assert!(server.get_tool("magistr_list").is_none());
}

// ── Startup validation tests (BF1.13) ──────────────────────────────────

#[test]
fn validate_executables_reports_missing() {
    let missing = validate_required_executables(&["cargo", "definitely_not_a_real_binary_xyz123"]);
    assert!(missing.is_empty() || missing.len() == 1);
    // cargo should be found (we're running in a Rust project)
    assert!(!missing.contains(&"cargo".to_string()));
    // fake binary should be missing
    assert!(missing.contains(&"definitely_not_a_real_binary_xyz123".to_string()));
}

#[test]
fn validate_executables_all_found() {
    // sh is universally available
    let missing = validate_required_executables(&["sh"]);
    assert!(missing.is_empty());
}

// ── magistr_gate tool tests ─────────────────────────────────────────────

fn setup_gate_project() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let magistr_dir = root.join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::write(
        magistr_dir.join("project.json"),
        serde_json::to_string_pretty(&json!({
            "project": { "name": "test", "description": "", "type": "cli" },
            "stack": {
                "language": "rust",
                "framework": "none",
                "package_manager": "cargo",
                "test_framework": "cargo-test",
                "linter": "clippy",
                "formatter": "rustfmt"
            },
            "quality_gate": {
                "check": "echo check-ok",
                "test": "echo test-ok",
                "lint": "echo lint-ok"
            },
            "format": {
                "extensions": ["rs"],
                "command": "echo formatted"
            },
            "spec": { "file": "ROADMAP.md", "unchecked_pattern": "" },
            "testing": { "companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside" },
            "reference_repos": [],
            "state_prefix": "test"
        }))
        .unwrap(),
    )
    .unwrap();
    (tmp, root)
}

#[tokio::test]
async fn gate_single_check_passes() {
    let (_tmp, root) = setup_gate_project();
    let server = build_server(root);
    let tool = server.get_tool("magistr_gate").unwrap();

    let result = tool.invoke(json!({ "gate": "check" })).await.unwrap();
    let text = match &result.content[0] {
        magistr_mcp::ToolContent::Text { text } => text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["gate"], "check");
    assert_eq!(parsed["passed"], true);
    assert!(parsed["output"].as_str().unwrap().contains("check-ok"));
    assert_eq!(parsed["exit_code"], 0);
    assert!(parsed["duration_ms"].as_u64().is_some());
}

#[tokio::test]
async fn gate_single_test_passes() {
    let (_tmp, root) = setup_gate_project();
    let server = build_server(root);
    let tool = server.get_tool("magistr_gate").unwrap();

    let result = tool.invoke(json!({ "gate": "test" })).await.unwrap();
    let text = match &result.content[0] {
        magistr_mcp::ToolContent::Text { text } => text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["gate"], "test");
    assert_eq!(parsed["passed"], true);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gate_all_runs_all_three() {
    let (_tmp, root) = setup_gate_project();
    let server = build_server(root);
    let tool = server.get_tool("magistr_gate").unwrap();

    let result = tool.invoke(json!({ "gate": "all" })).await.unwrap();
    let text = match &result.content[0] {
        magistr_mcp::ToolContent::Text { text } => text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["passed"], true);
    let gates = parsed["gates"].as_array().unwrap();
    assert_eq!(gates.len(), 3);
    assert_eq!(gates[0]["gate"], "check");
    assert_eq!(gates[1]["gate"], "test");
    assert_eq!(gates[2]["gate"], "lint");
}

#[tokio::test]
async fn gate_failing_command() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let magistr_dir = root.join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::write(
        magistr_dir.join("project.json"),
        serde_json::to_string_pretty(&json!({
            "project": { "name": "test", "description": "", "type": "cli" },
            "stack": {
                "language": "rust", "framework": "none",
                "package_manager": "cargo", "test_framework": "cargo-test",
                "linter": "clippy", "formatter": "rustfmt"
            },
            "quality_gate": {
                "check": "exit 1",
                "test": "echo test-ok",
                "lint": "echo lint-ok"
            },
            "format": { "extensions": ["rs"], "command": "echo fmt" },
            "spec": { "file": "ROADMAP.md", "unchecked_pattern": "" },
            "testing": { "companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside" },
            "reference_repos": [],
            "state_prefix": "test"
        }))
        .unwrap(),
    )
    .unwrap();

    let server = build_server(root);
    let tool = server.get_tool("magistr_gate").unwrap();

    let result = tool.invoke(json!({ "gate": "check" })).await.unwrap();
    let text = match &result.content[0] {
        magistr_mcp::ToolContent::Text { text } => text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["passed"], false);
    assert_eq!(parsed["exit_code"], 1);
}

#[tokio::test]
async fn gate_invalid_name_returns_error() {
    let (_tmp, root) = setup_gate_project();
    let server = build_server(root);
    let tool = server.get_tool("magistr_gate").unwrap();

    let result = tool.invoke(json!({ "gate": "bogus" })).await.unwrap();
    assert!(result.is_error.unwrap_or(false));
}

#[tokio::test]
async fn gate_timeout_expires() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let magistr_dir = root.join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::write(
        magistr_dir.join("project.json"),
        serde_json::to_string_pretty(&json!({
            "project": { "name": "test", "description": "", "type": "cli" },
            "stack": {
                "language": "rust", "framework": "none",
                "package_manager": "cargo", "test_framework": "cargo-test",
                "linter": "clippy", "formatter": "rustfmt"
            },
            "quality_gate": {
                "check": "sleep 10",
                "test": "",
                "lint": ""
            },
            "format": { "extensions": ["rs"], "command": "" },
            "spec": { "file": "ROADMAP.md", "unchecked_pattern": "" },
            "testing": { "companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside" },
            "reference_repos": [],
            "state_prefix": "test"
        }))
        .unwrap(),
    )
    .unwrap();

    let server = build_server(root);
    let tool = server.get_tool("magistr_gate").unwrap();

    let result = tool
        .invoke(json!({ "gate": "check", "timeout_secs": 1 }))
        .await
        .unwrap();
    let text = match &result.content[0] {
        magistr_mcp::ToolContent::Text { text } => text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["passed"], false);
    assert!(parsed["output"].as_str().unwrap().contains("timed out"));
}

// ── Custom gate tests ────────────────────────────────────────────────

fn setup_custom_gate_project() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let magistr_dir = root.join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::write(
        magistr_dir.join("project.json"),
        serde_json::to_string_pretty(&json!({
            "project": { "name": "test", "description": "", "type": "cli" },
            "stack": {
                "language": "rust", "framework": "none",
                "package_manager": "cargo", "test_framework": "cargo-test",
                "linter": "clippy", "formatter": "rustfmt"
            },
            "quality_gate": {
                "check": "echo check-ok",
                "test": "echo test-ok",
                "lint": "echo lint-ok"
            },
            "custom_gates": [
                { "name": "structure", "command": "echo structure-ok", "required": true },
                { "name": "audit", "command": "echo audit-ok", "required": false }
            ],
            "format": { "extensions": ["rs"], "command": "echo formatted" },
            "spec": { "file": "ROADMAP.md", "unchecked_pattern": "" },
            "testing": { "companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside" },
            "reference_repos": [],
            "state_prefix": "test"
        }))
        .unwrap(),
    )
    .unwrap();
    (tmp, root)
}

#[tokio::test]
async fn gate_custom_by_name() {
    let (_tmp, root) = setup_custom_gate_project();
    let server = build_server(root);
    let tool = server.get_tool("magistr_gate").unwrap();

    let result = tool.invoke(json!({ "gate": "structure" })).await.unwrap();
    let text = match &result.content[0] {
        magistr_mcp::ToolContent::Text { text } => text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["gate"], "structure");
    assert_eq!(parsed["passed"], true);
    assert!(parsed["output"].as_str().unwrap().contains("structure-ok"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gate_all_includes_required_custom_gates_and_leaves_the_rest_out() {
    let (_tmp, root) = setup_custom_gate_project();
    let server = build_server(root);
    let tool = server.get_tool("magistr_gate").unwrap();

    let result = tool.invoke(json!({ "gate": "all" })).await.unwrap();
    let text = match &result.content[0] {
        magistr_mcp::ToolContent::Text { text } => text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["passed"], true);
    let gates = parsed["gates"].as_array().unwrap();
    // check, test, lint + `structure` — and NOT `audit`, which is
    // registered `required: false`. This tool's own description says
    // "'all' runs standard + required custom gates"; it used to run every
    // custom gate, paying for a run whose red it then had to ignore.
    let names: Vec<&str> = gates.iter().map(|g| g["gate"].as_str().unwrap()).collect();
    assert_eq!(names, ["check", "test", "lint", "structure"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gate_all_does_not_pay_for_a_gate_it_would_have_to_ignore() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let magistr_dir = root.join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::write(
        magistr_dir.join("project.json"),
        serde_json::to_string_pretty(&json!({
            "project": { "name": "test", "description": "", "type": "cli" },
            "stack": {
                "language": "rust", "framework": "none",
                "package_manager": "cargo", "test_framework": "cargo-test",
                "linter": "clippy", "formatter": "rustfmt"
            },
            "quality_gate": {
                "check": "echo ok",
                "test": "echo ok",
                "lint": "echo ok"
            },
            "custom_gates": [
                { "name": "optional-check", "command": "exit 1", "required": false }
            ],
            "format": { "extensions": ["rs"], "command": "echo fmt" },
            "spec": { "file": "ROADMAP.md", "unchecked_pattern": "" },
            "testing": { "companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside" },
            "reference_repos": [],
            "state_prefix": "test"
        }))
        .unwrap(),
    )
    .unwrap();

    let server = build_server(root);
    let tool = server.get_tool("magistr_gate").unwrap();

    let result = tool.invoke(json!({ "gate": "all" })).await.unwrap();
    let text = match &result.content[0] {
        magistr_mcp::ToolContent::Text { text } => text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["passed"], true);
    let gates = parsed["gates"].as_array().unwrap();
    let names: Vec<&str> = gates.iter().map(|g| g["gate"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        ["check", "test", "lint"],
        "a gate registered not-required is expected to fail; `all` neither \
         runs it nor prints a FAIL row that reads like a failed iteration"
    );

    // It is still reachable by its own name — that is how it is watched.
    let result = tool
        .invoke(json!({ "gate": "optional-check" }))
        .await
        .unwrap();
    let text = match &result.content[0] {
        magistr_mcp::ToolContent::Text { text } => text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["passed"], false);
    assert_eq!(parsed["required"], false);
}

#[tokio::test]
async fn gate_unknown_includes_custom_names() {
    let (_tmp, root) = setup_custom_gate_project();
    let server = build_server(root);
    let tool = server.get_tool("magistr_gate").unwrap();

    let result = tool.invoke(json!({ "gate": "bogus" })).await.unwrap();
    assert!(result.is_error.unwrap_or(false));
    let text = match &result.content[0] {
        magistr_mcp::ToolContent::Text { text } => text,
        _ => panic!("expected text content"),
    };
    assert!(
        text.contains("structure"),
        "error should list custom gates: {text}"
    );
    assert!(
        text.contains("audit"),
        "error should list custom gates: {text}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gate_without_custom_gates() {
    // Config without custom_gates field should work fine
    let (_tmp, root) = setup_gate_project();
    let server = build_server(root);
    let tool = server.get_tool("magistr_gate").unwrap();

    let result = tool.invoke(json!({ "gate": "all" })).await.unwrap();
    let text = match &result.content[0] {
        magistr_mcp::ToolContent::Text { text } => text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["passed"], true);
    let gates = parsed["gates"].as_array().unwrap();
    assert_eq!(gates.len(), 3); // Only standard gates
}

// ── magistr_format tool tests ───────────────────────────────────────────

#[tokio::test]
async fn format_runs_command() {
    let (_tmp, root) = setup_gate_project();
    let server = build_server(root);
    let tool = server.get_tool("magistr_format").unwrap();

    let result = tool.invoke(json!({})).await.unwrap();
    let text = match &result.content[0] {
        magistr_mcp::ToolContent::Text { text } => text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["formatted"], true);
    assert!(parsed["output"].as_str().unwrap().contains("formatted"));
    assert_eq!(parsed["exit_code"], 0);
}

#[tokio::test]
async fn format_with_files() {
    let (_tmp, root) = setup_gate_project();
    let server = build_server(root);
    let tool = server.get_tool("magistr_format").unwrap();

    let result = tool
        .invoke(json!({ "files": ["src/main.rs", "src/lib.rs"] }))
        .await
        .unwrap();
    let text = match &result.content[0] {
        magistr_mcp::ToolContent::Text { text } => text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["formatted"], true);
    // Command should include the file paths
    assert!(parsed["command"].as_str().unwrap().contains("src/main.rs"));
}

#[tokio::test]
async fn format_no_command_configured() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let magistr_dir = root.join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::write(
        magistr_dir.join("project.json"),
        serde_json::to_string_pretty(&json!({
            "project": { "name": "test", "description": "", "type": "cli" },
            "stack": {
                "language": "rust", "framework": "none",
                "package_manager": "cargo", "test_framework": "cargo-test",
                "linter": "clippy", "formatter": "rustfmt"
            },
            "quality_gate": { "check": "", "test": "", "lint": "" },
            "format": { "extensions": [], "command": "" },
            "spec": { "file": "ROADMAP.md", "unchecked_pattern": "" },
            "testing": { "companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside" },
            "reference_repos": [],
            "state_prefix": "test"
        }))
        .unwrap(),
    )
    .unwrap();

    let server = build_server(root);
    let tool = server.get_tool("magistr_format").unwrap();

    let result = tool.invoke(json!({})).await.unwrap();
    let text = match &result.content[0] {
        magistr_mcp::ToolContent::Text { text } => text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["formatted"], false);
    assert!(
        parsed["output"]
            .as_str()
            .unwrap()
            .contains("no format command")
    );
}

#[tokio::test]
async fn format_shell_escapes_file_paths() {
    // Verify that file paths with shell metacharacters are escaped,
    // preventing shell injection (BF1.2).
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let magistr_dir = root.join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();
    // Use `echo` as format command so we can see what gets passed
    std::fs::write(
        magistr_dir.join("project.json"),
        serde_json::to_string_pretty(&json!({
            "project": { "name": "test", "description": "", "type": "cli" },
            "stack": {
                "language": "rust", "framework": "none",
                "package_manager": "cargo", "test_framework": "cargo-test",
                "linter": "clippy", "formatter": "rustfmt"
            },
            "quality_gate": { "check": "", "test": "", "lint": "" },
            "format": { "extensions": ["rs"], "command": "echo" },
            "spec": { "file": "ROADMAP.md", "unchecked_pattern": "" },
            "testing": { "companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside" },
            "reference_repos": [],
            "state_prefix": "test"
        }))
        .unwrap(),
    )
    .unwrap();

    let server = build_server(root);
    let tool = server.get_tool("magistr_format").unwrap();

    // Path with semicolon — would execute `rm -rf /` without escaping
    let result = tool
        .invoke(json!({ "files": ["file;rm -rf /", "hello world.rs", "file'quote.rs"] }))
        .await
        .unwrap();
    let text = match &result.content[0] {
        magistr_mcp::ToolContent::Text { text } => text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["formatted"], true);
    // The command should contain shell-escaped (single-quoted) file paths
    let cmd = parsed["command"].as_str().unwrap();
    // Paths with metacharacters must be single-quoted by shell_escape
    assert!(
        cmd.contains("'file;rm -rf /'"),
        "semicolon path must be single-quoted: {cmd}"
    );
    assert!(
        cmd.contains("'hello world.rs'"),
        "space in path must be single-quoted: {cmd}"
    );
}

// ── Subprocess safety tests (BF1.3, BF1.7) ─────────────────────────────

#[tokio::test]
async fn gate_command_does_not_hang_on_stdin() {
    // BF1.7: stdin must be null so commands that read stdin get immediate EOF
    let dir = std::env::current_dir().unwrap();
    let result = run_gate_command("cat", &dir, 5, |_| {}).await.unwrap();
    // cat with stdin null exits successfully with empty output
    assert_eq!(result["passed"], true);
}

#[tokio::test]
async fn gate_command_timeout_kills_child() {
    // BF1.3: verify the child process is actually killed on timeout
    let dir = std::env::current_dir().unwrap();
    let result = run_gate_command("sleep 60", &dir, 1, |_| {}).await.unwrap();
    assert_eq!(result["passed"], false);
    assert!(result["output"].as_str().unwrap().contains("timed out"));
}

// ── Output truncation tests (BF1.6) ─────────────────────────────────────

#[tokio::test]
async fn gate_command_truncates_large_output() {
    // BF1.6: large output gets truncated
    let dir = std::env::current_dir().unwrap();
    // Generate output larger than crate::utils::gate::MAX_OUTPUT_BYTES
    let cmd = format!(
        "python3 -c \"print('x' * {})\"",
        crate::utils::gate::MAX_OUTPUT_BYTES + 10_000
    );
    let result = run_gate_command(&cmd, &dir, 30, |_| {}).await.unwrap();
    let output = result["output"].as_str().unwrap();
    assert!(
        output.len() <= crate::utils::gate::MAX_OUTPUT_BYTES + 200, // allow for marker text
        "output should be truncated: got {} bytes",
        output.len()
    );
    assert!(
        output.contains("[... truncated,"),
        "should contain truncation marker"
    );
}

// ── Format exclusion tests (BF1.1) ──────────────────────────────────────

#[test]
fn build_find_format_command_basic() {
    let cmd = build_find_format_command(
        "ruff format {file}",
        &["py".into()],
        &[".venv".into(), "__pycache__".into()],
    );
    assert!(cmd.contains("-path './.venv'"), "should prune .venv: {cmd}");
    assert!(
        cmd.contains("-path './__pycache__'"),
        "should prune __pycache__: {cmd}"
    );
    assert!(cmd.contains("-name '*.py'"), "should filter .py: {cmd}");
    assert!(cmd.contains("xargs"), "should pipe to xargs: {cmd}");
    assert!(
        cmd.contains("ruff format {}"),
        "should replace {{file}} with {{}}: {cmd}"
    );
}

#[test]
fn build_find_format_command_no_placeholder() {
    let cmd = build_find_format_command("cargo fmt", &["rs".into()], &["target".into()]);
    assert!(
        cmd.contains("xargs -0 cargo fmt"),
        "should append files to command: {cmd}"
    );
}

#[test]
fn build_find_format_command_multiple_extensions() {
    let cmd = build_find_format_command(
        "clang-format -i {file}",
        &["cpp".into(), "hpp".into(), "c".into()],
        &["build".into()],
    );
    assert!(cmd.contains("-name '*.cpp'"), "{cmd}");
    assert!(cmd.contains("-name '*.hpp'"), "{cmd}");
    assert!(cmd.contains("-name '*.c'"), "{cmd}");
}

// ── Parallel gate execution tests (BF1.5) ────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gate_all_runs_concurrently() {
    // BF1.5: verify "all" gate runs concurrently by checking total time
    // is closer to max(gate_time) than sum(gate_time)
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let magistr_dir = root.join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::write(
        magistr_dir.join("project.json"),
        serde_json::to_string_pretty(&json!({
            "project": { "name": "test", "description": "", "type": "cli" },
            "stack": {
                "language": "rust", "framework": "none",
                "package_manager": "cargo", "test_framework": "cargo-test",
                "linter": "clippy", "formatter": "rustfmt"
            },
            "quality_gate": {
                "check": "sleep 0.5 && echo check-ok",
                "test": "sleep 0.5 && echo test-ok",
                "lint": "sleep 0.5 && echo lint-ok"
            },
            "format": { "extensions": ["rs"], "command": "echo formatted" },
            "spec": { "file": "ROADMAP.md", "unchecked_pattern": "" },
            "testing": { "companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside" },
            "reference_repos": [],
            "state_prefix": "test"
        }))
        .unwrap(),
    )
    .unwrap();

    let server = build_server(root);
    let tool = server.get_tool("magistr_gate").unwrap();

    let start = std::time::Instant::now();
    let result = tool.invoke(json!({ "gate": "all" })).await.unwrap();
    let elapsed = start.elapsed();

    let text = match &result.content[0] {
        magistr_mcp::ToolContent::Text { text } => text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["passed"], true);

    // Sequential would take >= 1.5s (3 × 0.5s), parallel should be < 1.2s
    assert!(
        elapsed.as_secs_f64() < 1.2,
        "gates should run in parallel: took {:.2}s (expected < 1.2s)",
        elapsed.as_secs_f64()
    );
}

// ── wb3: junit report path for nextest commands ───────────────────────────

#[test]
fn nextest_junit_report_path_follows_the_profile() {
    use crate::utils::gate::nextest_junit_report_path;
    assert_eq!(
        nextest_junit_report_path("cargo nextest run").as_deref(),
        Some("target/nextest/default/junit.xml")
    );
    assert_eq!(
        nextest_junit_report_path("cargo nextest run -P ci --workspace").as_deref(),
        Some("target/nextest/ci/junit.xml")
    );
    assert_eq!(
        nextest_junit_report_path("cargo nextest run --profile=slow").as_deref(),
        Some("target/nextest/slow/junit.xml")
    );
    assert_eq!(
        nextest_junit_report_path("/usr/bin/cargo +stable nextest run -Pbench").as_deref(),
        Some("target/nextest/bench/junit.xml")
    );
    assert_eq!(nextest_junit_report_path("cargo test"), None);
    assert_eq!(nextest_junit_report_path("npm test"), None);
}

// ── format command resolution (format-file-placeholder) ─────────────────

fn fmt_config(command: &str, extensions: &[&str], exclusions: &[&str]) -> FormatConfig {
    FormatConfig {
        command: command.into(),
        extensions: extensions.iter().map(|s| (*s).to_string()).collect(),
        exclusions: exclusions.iter().map(|s| (*s).to_string()).collect(),
    }
}

#[test]
fn resolve_expands_a_placeholder_command_over_extensions_when_no_files_are_given() {
    let fmt = fmt_config("rustfmt {file}", &["rs"], &["target"]);
    for files in [None, Some(Vec::<&str>::new())] {
        let cmd = resolve_format_command(&fmt, files.as_deref()).expect("resolves");
        assert!(cmd.starts_with("find . "), "{cmd}");
        assert!(cmd.contains("-name '*.rs'"), "{cmd}");
        assert!(cmd.contains("-path './target'"), "{cmd}");
        assert!(cmd.contains("-path './.git'"), "always prunes .git: {cmd}");
        assert!(cmd.ends_with("xargs -0 -I {} rustfmt {}"), "{cmd}");
        assert!(
            !cmd.contains("{file}"),
            "never runs the placeholder literally: {cmd}"
        );
    }
}

#[test]
fn resolve_expands_even_without_exclusions() {
    let fmt = fmt_config("ruff format {file}", &["py"], &[]);
    let cmd = resolve_format_command(&fmt, None).expect("resolves");
    assert!(cmd.contains("-name '*.py'"), "{cmd}");
    assert!(cmd.ends_with("xargs -0 -I {} ruff format {}"), "{cmd}");
}

#[test]
fn resolve_refuses_a_placeholder_command_with_no_extensions_and_no_files() {
    let fmt = fmt_config("rustfmt {file}", &[], &[]);
    let err = resolve_format_command(&fmt, None).expect_err("nothing to expand over");
    assert!(err.contains("{file}"), "{err}");
    assert!(err.contains("extensions"), "{err}");
}

#[test]
fn resolve_runs_a_whole_project_command_literally_when_no_files_are_given() {
    let fmt = fmt_config("cargo fmt", &["rs"], &["target"]);
    assert_eq!(resolve_format_command(&fmt, None).unwrap(), "cargo fmt");
    assert_eq!(
        resolve_format_command(&fmt, Some(&[])).unwrap(),
        "cargo fmt"
    );
}

#[test]
fn resolve_substitutes_the_placeholder_per_file() {
    let fmt = fmt_config("rustfmt {file}", &["rs"], &["target"]);
    let cmd = resolve_format_command(&fmt, Some(&["src/main.rs", "hello world.rs"])).unwrap();
    assert_eq!(cmd, "rustfmt src/main.rs && rustfmt 'hello world.rs'");
}

#[test]
fn resolve_appends_files_to_a_command_without_placeholder() {
    let fmt = fmt_config("prettier --write", &["ts"], &[]);
    let cmd = resolve_format_command(&fmt, Some(&["a.ts", "b c.ts"])).unwrap();
    assert_eq!(cmd, "prettier --write a.ts 'b c.ts'");
}

/// The live ts4d failure: `magistr_format` with no file list on a project
/// whose command carries `{file}` used to run `rustfmt {file}` as text.
#[tokio::test]
async fn format_without_files_expands_a_placeholder_command_over_the_tree() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let magistr_dir = root.join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("target")).unwrap();
    std::fs::write(root.join("src/a.txt"), "a").unwrap();
    std::fs::write(root.join("src/b.txt"), "b").unwrap();
    std::fs::write(root.join("target/skip.txt"), "x").unwrap();
    std::fs::write(root.join("src/not-me.rs"), "fn main() {}").unwrap();
    std::fs::write(
        magistr_dir.join("project.json"),
        serde_json::to_string_pretty(&json!({
            "project": { "name": "test", "description": "", "type": "cli" },
            "stack": {
                "language": "rust", "framework": "none",
                "package_manager": "cargo", "test_framework": "cargo-test",
                "linter": "clippy", "formatter": "rustfmt"
            },
            "quality_gate": { "check": "", "test": "", "lint": "" },
            "format": {
                "extensions": ["txt"],
                "command": "cp {file} {file}.formatted",
                "exclusions": ["target"]
            },
            "spec": { "file": "ROADMAP.md", "unchecked_pattern": "" },
            "testing": { "companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside" },
            "reference_repos": [],
            "state_prefix": "test"
        }))
        .unwrap(),
    )
    .unwrap();

    let server = build_server(root.clone());
    let tool = server.get_tool("magistr_format").unwrap();

    for input in [json!({}), json!({ "files": [] })] {
        let result = tool.invoke(input.clone()).await.unwrap();
        let text = match &result.content[0] {
            magistr_mcp::ToolContent::Text { text } => text,
            _ => panic!("expected text content"),
        };
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["formatted"], true, "{input}: {parsed}");
        assert_eq!(parsed["exit_code"], 0, "{input}: {parsed}");
        let cmd = parsed["command"].as_str().unwrap();
        assert!(
            !cmd.contains("{file}"),
            "{input}: ran the placeholder: {cmd}"
        );
    }
    assert!(root.join("src/a.txt.formatted").exists());
    assert!(root.join("src/b.txt.formatted").exists());
    assert!(
        !root.join("target/skip.txt.formatted").exists(),
        "exclusions pruned"
    );
    assert!(
        !root.join("src/not-me.rs.formatted").exists(),
        "extensions honoured"
    );
}

#[tokio::test]
async fn format_with_files_substitutes_the_placeholder_per_file() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let magistr_dir = root.join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::write(root.join("one.txt"), "1").unwrap();
    std::fs::write(root.join("two.txt"), "2").unwrap();
    std::fs::write(
        magistr_dir.join("project.json"),
        serde_json::to_string_pretty(&json!({
            "project": { "name": "test", "description": "", "type": "cli" },
            "stack": {
                "language": "rust", "framework": "none",
                "package_manager": "cargo", "test_framework": "cargo-test",
                "linter": "clippy", "formatter": "rustfmt"
            },
            "quality_gate": { "check": "", "test": "", "lint": "" },
            "format": { "extensions": ["txt"], "command": "cp {file} {file}.formatted" },
            "spec": { "file": "ROADMAP.md", "unchecked_pattern": "" },
            "testing": { "companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside" },
            "reference_repos": [],
            "state_prefix": "test"
        }))
        .unwrap(),
    )
    .unwrap();

    let server = build_server(root.clone());
    let tool = server.get_tool("magistr_format").unwrap();
    let result = tool
        .invoke(json!({ "files": ["one.txt", "two.txt"] }))
        .await
        .unwrap();
    let text = match &result.content[0] {
        magistr_mcp::ToolContent::Text { text } => text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["formatted"], true, "{parsed}");
    assert!(root.join("one.txt.formatted").exists());
    assert!(root.join("two.txt.formatted").exists());
}

#[test]
fn rust_preset_seeds_a_whole_project_formatter() {
    let preset: serde_json::Value =
        serde_json::from_str(include_str!("../../../magistr-core/presets/rust.json")).unwrap();
    assert_eq!(preset["format"]["command"], "cargo fmt");
}

/// The `shar-remaster` failure, end to end: eight archetypes, every one of
/// them carrying the *project's* workspace-wide command, which is written
/// relative to the project root. Run from each archetype's own root that
/// command cannot resolve its own path, so all 29 recorded `magistr_format`
/// calls in that project failed with rustfmt's help text and no mention of
/// the working directory. It is one formatter, so it is one run, at the root.
#[tokio::test]
async fn a_project_command_copied_onto_every_archetype_runs_once_at_the_root() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    std::fs::create_dir_all(root.join(".magistr")).unwrap();
    for dir in ["rust", "rust/apps/shar", "rust/crates/sim"] {
        std::fs::create_dir_all(root.join(dir)).unwrap();
    }
    // Succeeds only in the directory the marker's relative path resolves in.
    let command = "test -d rust && echo formatted";
    let archetype = |id: &str, arch_root: &str| {
        json!({
            "id": id, "name": id, "language": "rust", "root": arch_root,
            "role": "unknown",
            "quality_gate": {"check": "", "test": "", "lint": ""},
            "format": {"command": command, "extensions": ["rs"], "exclusions": []},
            "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
            "source_patterns": [], "test_patterns": [], "config_patterns": []
        })
    };
    std::fs::write(
        root.join(".magistr/project.json"),
        serde_json::to_string_pretty(&json!({
            "project": { "name": "test", "description": "", "type": "cli" },
            "stack": {
                "language": "rust", "framework": "none",
                "package_manager": "cargo", "test_framework": "cargo-test",
                "linter": "clippy", "formatter": "rustfmt"
            },
            "quality_gate": { "check": "", "test": "", "lint": "" },
            "format": { "extensions": ["rs"], "command": command },
            "spec": { "file": "ROADMAP.md", "unchecked_pattern": "" },
            "testing": { "companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside" },
            "reference_repos": [],
            "state_prefix": "test",
            "archetypes": [
                archetype("rust", "rust"),
                archetype("shar", "rust/apps/shar"),
                archetype("sim", "rust/crates/sim"),
            ]
        }))
        .unwrap(),
    )
    .unwrap();

    let server = build_server(root.clone());
    let tool = server.get_tool("magistr_format").unwrap();
    let result = tool.invoke(json!({})).await.unwrap();
    let text = match &result.content[0] {
        magistr_mcp::ToolContent::Text { text } => text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["formatted"], true, "{parsed}");
    let arches = parsed["archetypes"].as_array().unwrap();
    // Every archetype still gets its own entry — the response shape is the
    // one callers have always read — and every one of them succeeded.
    assert_eq!(arches.len(), 3, "{parsed}");
    for a in arches {
        assert_eq!(a["formatted"], true, "{a}");
        assert_eq!(a["exit_code"], 0, "{a}");
    }
}

/// A failed archetype format says which directory it ran in. Without it the
/// answer is the formatter's own error — "manifest path `rust/Cargo.toml`
/// does not exist" — with nothing to suggest the working directory is why.
#[tokio::test]
async fn a_failed_archetype_format_names_the_directory_it_ran_in() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    std::fs::create_dir_all(root.join(".magistr")).unwrap();
    std::fs::create_dir_all(root.join("web")).unwrap();
    std::fs::write(
        root.join(".magistr/project.json"),
        serde_json::to_string_pretty(&json!({
            "project": { "name": "test", "description": "", "type": "cli" },
            "stack": {
                "language": "rust", "framework": "none",
                "package_manager": "cargo", "test_framework": "cargo-test",
                "linter": "clippy", "formatter": "rustfmt"
            },
            "quality_gate": { "check": "", "test": "", "lint": "" },
            "format": { "extensions": ["rs"], "command": "true" },
            "spec": { "file": "ROADMAP.md", "unchecked_pattern": "" },
            "testing": { "companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside" },
            "reference_repos": [],
            "state_prefix": "test",
            "archetypes": [{
                "id": "web", "name": "web", "language": "ts", "root": "web",
                "role": "unknown",
                "quality_gate": {"check": "", "test": "", "lint": ""},
                "format": {"command": "exit 7", "extensions": ["ts"], "exclusions": []},
                "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
                "source_patterns": [], "test_patterns": [], "config_patterns": []
            }]
        }))
        .unwrap(),
    )
    .unwrap();

    let server = build_server(root.clone());
    let tool = server.get_tool("magistr_format").unwrap();
    let result = tool.invoke(json!({})).await.unwrap();
    let text = match &result.content[0] {
        magistr_mcp::ToolContent::Text { text } => text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    let web = &parsed["archetypes"][0];
    assert_eq!(web["formatted"], false, "{parsed}");
    assert_eq!(
        web["dir"].as_str(),
        Some(root.join("web").display().to_string().as_str()),
        "a failure names its working directory: {web}"
    );
}

/// Targeting one archetype takes the same rule as formatting them all — the
/// recorded `shar-remaster` failure was `magistr_format(archetype: "rust")`,
/// so the filtered path is the one a user hits first.
#[tokio::test]
async fn targeting_one_archetype_runs_where_the_whole_project_run_would() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    std::fs::create_dir_all(root.join(".magistr")).unwrap();
    std::fs::create_dir_all(root.join("rust/apps/shar")).unwrap();
    let command = "test -d rust && echo formatted";
    std::fs::write(
        root.join(".magistr/project.json"),
        serde_json::to_string_pretty(&json!({
            "project": { "name": "test", "description": "", "type": "cli" },
            "stack": {
                "language": "rust", "framework": "none",
                "package_manager": "cargo", "test_framework": "cargo-test",
                "linter": "clippy", "formatter": "rustfmt"
            },
            "quality_gate": { "check": "", "test": "", "lint": "" },
            "format": { "extensions": ["rs"], "command": command },
            "spec": { "file": "ROADMAP.md", "unchecked_pattern": "" },
            "testing": { "companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside" },
            "reference_repos": [],
            "state_prefix": "test",
            "archetypes": [{
                "id": "shar", "name": "shar", "language": "rust", "root": "rust/apps/shar",
                "role": "unknown",
                "quality_gate": {"check": "", "test": "", "lint": ""},
                "format": {"command": command, "extensions": ["rs"], "exclusions": []},
                "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
                "source_patterns": [], "test_patterns": [], "config_patterns": []
            }]
        }))
        .unwrap(),
    )
    .unwrap();

    let server = build_server(root.clone());
    let tool = server.get_tool("magistr_format").unwrap();
    let result = tool.invoke(json!({ "archetype": "shar" })).await.unwrap();
    let text = match &result.content[0] {
        magistr_mcp::ToolContent::Text { text } => text,
        _ => panic!("expected text content"),
    };
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["formatted"], true, "{parsed}");
    assert_eq!(parsed["exit_code"], 0, "{parsed}");
}
