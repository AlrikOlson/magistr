//! CLI integration tests — run the real `magistr` binary against temp directories.

#[path = "support/magistr_command.rs"]
mod magistr_command;

use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[cfg(unix)]
use std::process::Stdio;

fn write_file(dir: &Path, name: &str, content: &str) {
    if let Some(parent) = Path::new(name).parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir.join(parent)).unwrap();
    }
    std::fs::write(dir.join(name), content).unwrap();
}

fn magistr() -> Command {
    Command::from_std(magistr_command::magistr_command())
}

#[test]
fn explicit_environment_is_still_read_by_the_real_cli() {
    let dir = TempDir::new().unwrap();
    magistr()
        .env("MAGISTR_BUDGET", "invalid-budget")
        .args(["detect", dir.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid-budget"));
    magistr()
        .env("MAGISTR_BUDGET", "5.00")
        .args(["detect", dir.path().to_str().unwrap()])
        .assert()
        .success();
}

// ── magistr detect ──────────────────────────────────────────────────────────

#[test]
fn detect_rust_project() {
    let dir = TempDir::new().unwrap();
    write_file(
        dir.path(),
        "Cargo.toml",
        r#"[package]
name = "test-app"
version = "0.1.0"
edition = "2021"
"#,
    );

    magistr()
        .args(["detect", &dir.path().display().to_string()])
        .assert()
        .success()
        .stdout(predicate::str::contains("rust"));
}

#[test]
fn detect_typescript_project() {
    let dir = TempDir::new().unwrap();
    write_file(
        dir.path(),
        "package.json",
        r#"{"name": "test-app", "devDependencies": {"typescript": "^5.0"}}"#,
    );
    write_file(dir.path(), "tsconfig.json", "{}");

    magistr()
        .args(["detect", &dir.path().display().to_string()])
        .assert()
        .success()
        .stdout(predicate::str::contains("typescript"));
}

#[test]
fn detect_python_project() {
    let dir = TempDir::new().unwrap();
    write_file(
        dir.path(),
        "pyproject.toml",
        r#"[project]
name = "test-app"
"#,
    );

    magistr()
        .args(["detect", &dir.path().display().to_string()])
        .assert()
        .success()
        .stdout(predicate::str::contains("python"));
}

#[test]
fn detect_json_output() {
    let dir = TempDir::new().unwrap();
    write_file(
        dir.path(),
        "Cargo.toml",
        r#"[package]
name = "json-test"
version = "0.1.0"
edition = "2021"
"#,
    );

    let output = magistr()
        .args(["--json", "detect", &dir.path().display().to_string()])
        .assert()
        .success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["data"]["language"], "rust");
}

#[test]
fn detect_empty_dir_returns_unknown() {
    let dir = TempDir::new().unwrap();

    magistr()
        .args(["detect", &dir.path().display().to_string()])
        .assert()
        .success()
        .stdout(predicate::str::contains("unknown"));
}

// ── magistr init ────────────────────────────────────────────────────────────

#[test]
fn init_creates_claude_dir() {
    let dir = TempDir::new().unwrap();
    write_file(
        dir.path(),
        "Cargo.toml",
        r#"[package]
name = "init-test"
version = "0.1.0"
edition = "2021"
"#,
    );

    magistr()
        .args(["--no-tui", "init", &dir.path().display().to_string()])
        .assert()
        .success();

    // Verify directories were created with expected files
    assert!(dir.path().join(".claude").is_dir());
    assert!(dir.path().join(".magistr/project.json").is_file());
    assert!(dir.path().join(".claude/settings.json").is_file());
}

#[test]
fn init_generates_valid_project_json() {
    let dir = TempDir::new().unwrap();
    write_file(
        dir.path(),
        "Cargo.toml",
        r#"[package]
name = "config-test"
version = "0.1.0"
edition = "2021"
"#,
    );

    magistr()
        .args(["--no-tui", "init", &dir.path().display().to_string()])
        .assert()
        .success();

    let content = std::fs::read_to_string(dir.path().join(".magistr/project.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed["stack"]["language"], "rust");
    assert_eq!(parsed["project"]["name"], "config-test");
}

#[test]
fn init_is_idempotent_with_existing_claude_dir() {
    let dir = TempDir::new().unwrap();
    write_file(dir.path(), "Cargo.toml", "[package]\nname = \"test\"\n");
    std::fs::create_dir_all(dir.path().join(".claude")).unwrap();

    // init should succeed even when .claude/ already exists (idempotent update).
    magistr()
        .args(["--no-tui", "init", &dir.path().display().to_string()])
        .assert()
        .success()
        .stderr(predicate::str::contains("scaffolded successfully"));
}

// ── magistr migrate ─────────────────────────────────────────────────────────

#[test]
fn migrate_no_claude_dir() {
    let dir = TempDir::new().unwrap();

    magistr()
        .args(["migrate", &dir.path().display().to_string()])
        .assert()
        .success()
        .stderr(predicate::str::contains("up to date"));
}

#[test]
fn migrate_detects_missing_project_json() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join(".claude")).unwrap();

    magistr()
        .args(["migrate", &dir.path().display().to_string()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("project.json"));
}

#[test]
fn migrate_detects_bash_hooks() {
    let dir = TempDir::new().unwrap();
    let claude_dir = dir.path().join(".claude");
    let hooks_dir = claude_dir.join("hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(claude_dir.join("project.json"), "{}").unwrap();
    std::fs::write(hooks_dir.join("validate.sh"), "#!/bin/bash\ncargo test").unwrap();

    magistr()
        .args(["migrate", &dir.path().display().to_string()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("bash shebang"));
}

#[test]
fn migrate_passes_clean_config() {
    let dir = TempDir::new().unwrap();
    let claude_dir = dir.path().join(".claude");
    let magistr_dir = dir.path().join(".magistr");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::create_dir_all(&magistr_dir).unwrap();
    // A clean config declares its roadmap backend; one that predates the key
    // is auto-migrated (see `migrate_writes_a_missing_roadmap_backend`).
    std::fs::write(
        magistr_dir.join("project.json"),
        r#"{"roadmap_backend":"think-and-ship"}"#,
    )
    .unwrap();

    magistr()
        .args(["migrate", &dir.path().display().to_string()])
        .assert()
        .success()
        .stderr(predicate::str::contains("up to date"));
}

#[test]
fn migrate_writes_a_missing_roadmap_backend() {
    let dir = TempDir::new().unwrap();
    let magistr_dir = dir.path().join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::write(magistr_dir.join("project.json"), "{}").unwrap();

    magistr()
        .args(["migrate", &dir.path().display().to_string()])
        .assert()
        .success()
        .stderr(predicate::str::contains("roadmap_backend"));
    let written = std::fs::read_to_string(magistr_dir.join("project.json")).unwrap();
    assert!(
        written.contains("\"roadmap_backend\": \"think-and-ship\""),
        "{written}"
    );
}

// ── magistr help / version ──────────────────────────────────────────────────

#[test]
fn help_flag_works() {
    magistr()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("AI-powered development toolkit"));
}

#[test]
fn version_flag_works() {
    magistr()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("magistr"));
}

// ── magistr completions ─────────────────────────────────────────────────────

#[test]
fn completions_bash() {
    magistr()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("complete"));
}

#[test]
fn completions_zsh() {
    magistr()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("compdef"));
}

// ── magistr man-pages ───────────────────────────────────────────────────────

#[test]
fn man_pages_generates_file() {
    let dir = TempDir::new().unwrap();

    magistr()
        .args(["man-pages", &dir.path().display().to_string()])
        .assert()
        .success();

    assert!(dir.path().join("magistr.1").is_file());
}

// ── magistr export / import ─────────────────────────────────────────────

#[test]
fn export_requires_project_json() {
    let dir = TempDir::new().unwrap();

    magistr()
        .args(["export", "--no-tui", &dir.path().display().to_string()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("project.json"));
}

#[test]
fn export_creates_preset_file() {
    let dir = TempDir::new().unwrap();

    // First init a Rust project so project.json exists
    write_file(
        dir.path(),
        "Cargo.toml",
        r#"[package]
name = "test-proj"
version = "0.1.0"
edition = "2021"
"#,
    );
    write_file(dir.path(), "src/main.rs", "fn main() {}");

    magistr()
        .args(["init", "--no-tui", &dir.path().display().to_string()])
        .assert()
        .success();

    // Now export
    magistr()
        .args(["export", "--no-tui", &dir.path().display().to_string()])
        .assert()
        .success()
        .stderr(predicate::str::contains("Exported preset"));

    let preset_path = dir.path().join(".magistr-preset.toml");
    assert!(preset_path.is_file());

    let content = std::fs::read_to_string(&preset_path).unwrap();
    assert!(content.contains("name = \"test-proj\""));
    assert!(content.contains("[stack]"));
    assert!(content.contains("language = \"rust\""));
}

#[test]
fn export_import_round_trip() {
    let src_dir = TempDir::new().unwrap();
    let dst_dir = TempDir::new().unwrap();

    // Set up source project
    write_file(
        src_dir.path(),
        "Cargo.toml",
        r#"[package]
name = "source-proj"
version = "0.1.0"
edition = "2021"
"#,
    );
    write_file(src_dir.path(), "src/main.rs", "fn main() {}");

    // Init source
    magistr()
        .args(["init", "--no-tui", &src_dir.path().display().to_string()])
        .assert()
        .success();

    // Export from source
    let preset_path = src_dir.path().join("team-preset.toml");
    magistr()
        .args([
            "export",
            "--no-tui",
            &src_dir.path().display().to_string(),
            "-o",
            &preset_path.display().to_string(),
        ])
        .assert()
        .success();

    // Import into destination
    magistr()
        .args([
            "import",
            "--no-tui",
            &preset_path.display().to_string(),
            &dst_dir.path().display().to_string(),
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("Imported preset"));

    // Verify scaffold was created
    assert!(dst_dir.path().join(".claude").is_dir());
    assert!(dst_dir.path().join(".magistr/project.json").is_file());
}

#[test]
fn init_from_preset() {
    let src_dir = TempDir::new().unwrap();
    let dst_dir = TempDir::new().unwrap();

    // Set up source and export
    write_file(
        src_dir.path(),
        "Cargo.toml",
        r#"[package]
name = "from-proj"
version = "0.1.0"
edition = "2021"
"#,
    );
    write_file(src_dir.path(), "src/main.rs", "fn main() {}");

    magistr()
        .args(["init", "--no-tui", &src_dir.path().display().to_string()])
        .assert()
        .success();

    let preset_path = src_dir.path().join(".magistr-preset.toml");
    magistr()
        .args(["export", "--no-tui", &src_dir.path().display().to_string()])
        .assert()
        .success();

    // Init destination from preset
    magistr()
        .args([
            "init",
            "--no-tui",
            "--from",
            &preset_path.display().to_string(),
            &dst_dir.path().display().to_string(),
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("Initialized from file preset"));

    assert!(dst_dir.path().join(".claude").is_dir());
    assert!(dst_dir.path().join(".magistr/project.json").is_file());
}

// ── SIGINT (Ctrl+C) signal handler ──────────────────────────────────

/// Verify that magistr's SIGINT handler produces exit code 130 (128 + SIGINT).
///
/// This proves the tokio signal handler in `install_signal_handler()` fires
/// and performs terminal cleanup before exit. Without it, the default SIGINT
/// behavior would leave mouse tracking enabled, corrupting the terminal.
///
/// Uses `validate --ci on magistr's own repo (which runs cargo check/test/clippy
/// and takes several seconds), giving us a reliable window to send SIGINT.
#[cfg(unix)]
#[test]
fn sigint_exits_with_code_130() {
    use std::time::Duration;

    // Run validate on magistr's own repo — cargo check/test/clippy takes seconds
    let project_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

    let mut child = magistr_command::magistr_command()
        .args(["validate", "--ci", project_dir])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn magistr validate");

    // Give the process time to start the tokio runtime and install the signal handler
    std::thread::sleep(Duration::from_millis(500));

    // Send SIGINT via kill command (avoids libc dev-dependency)
    std::process::Command::new("kill")
        .args(["-2", &child.id().to_string()])
        .status()
        .expect("failed to send SIGINT");

    let status = child.wait().expect("failed to wait for child");

    // Exit code 130 = our signal handler fired (128 + SIGINT(2))
    // Without the handler, SIGINT on Unix terminates the process via signal
    // disposition (no exit code — status.code() returns None, only
    // status.signal() returns Some(2)). Exit code 130 proves the handler ran.
    assert_eq!(
        status.code(),
        Some(130),
        "SIGINT should produce exit code 130 (signal handler must fire). \
         Got code={:?} signal={:?}",
        status.code(),
        std::os::unix::process::ExitStatusExt::signal(&status),
    );
}

// ── NO_COLOR support ───────────────────────────────────────────────

#[test]
fn no_color_produces_clean_output() {
    let dir = TempDir::new().unwrap();
    write_file(
        dir.path(),
        "Cargo.toml",
        r#"[package]
name = "test-proj"
version = "0.1.0"
edition = "2021"
"#,
    );
    write_file(dir.path(), "src/main.rs", "fn main() {}");

    let output = magistr()
        .env("NO_COLOR", "1")
        .args(["detect", "--no-tui", &dir.path().display().to_string()])
        .output()
        .expect("failed to run magistr detect");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // No ANSI escape sequences anywhere in the output
    assert!(
        !stderr.contains("\x1b["),
        "stderr contains ANSI escapes with NO_COLOR=1: {stderr}"
    );
    assert!(
        !stdout.contains("\x1b["),
        "stdout contains ANSI escapes with NO_COLOR=1: {stdout}"
    );
}
