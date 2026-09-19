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

/// A project whose `check` gate is `gate`, for the interrupt fixtures: the
/// gate writes its shell's pid to `gate.pid` so the test can wait for it
/// to be running and prove it is gone afterwards.
#[cfg(unix)]
fn interrupt_project(gate: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    let config = serde_json::json!({
        "project": {"name":"interrupt-fixture"},
        "stack": {"language":"rust", "framework":"none", "package_manager":"cargo",
            "test_framework":"cargo-test", "linter":"clippy", "formatter":"rustfmt"},
        "quality_gate": {"check": gate, "test":"true", "lint":"true"},
        "format": {"command":"true", "extensions":[]},
        "testing": {"companion_pattern":"{name}.tests.{ext}", "test_dir":"alongside"},
        "reference_repos":[], "state_prefix":"interrupt-fixture",
        "custom_gates": []
    });
    std::fs::create_dir_all(dir.path().join(".magistr")).unwrap();
    std::fs::write(
        dir.path().join(".magistr/project.json"),
        serde_json::to_vec_pretty(&config).unwrap(),
    )
    .unwrap();
    dir
}

/// Spawn `magistr validate --ci` on the project and wait until its gate
/// child is running; answers the child and the gate's pid.
#[cfg(unix)]
fn spawn_validate_until_gate_runs(dir: &TempDir) -> (std::process::Child, i32) {
    use std::time::{Duration, Instant};
    let child = magistr_command::magistr_command()
        .args(["validate", "--ci", &dir.path().display().to_string()])
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join("config"))
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn magistr validate");
    let pid_file = dir.path().join("gate.pid");
    let deadline = Instant::now() + Duration::from_secs(20);
    let gate_pid = loop {
        if let Ok(text) = std::fs::read_to_string(&pid_file)
            && let Ok(pid) = text.trim().parse::<i32>()
        {
            break pid;
        }
        assert!(Instant::now() < deadline, "the gate never started");
        std::thread::sleep(Duration::from_millis(20));
    };
    (child, gate_pid)
}

#[cfg(unix)]
fn signal(pid: u32, signal: &str) {
    std::process::Command::new("kill")
        .args([signal, &pid.to_string()])
        .status()
        .expect("failed to send the signal");
}

#[cfg(unix)]
fn process_is_gone(pid: i32) -> bool {
    !std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .expect("failed to probe the process")
        .success()
}

/// The first SIGINT is graceful: the running gate child is ended and
/// joined, the command finishes its own report, the process exits 130 and
/// the emergency path is never taken.
#[cfg(unix)]
#[test]
fn first_sigint_settles_the_running_gate_child_and_exits_130_without_the_emergency_exit() {
    use std::time::{Duration, Instant};
    let dir = interrupt_project("echo $$ > gate.pid; sleep 30");
    let (child, gate_pid) = spawn_validate_until_gate_runs(&dir);
    let interrupted_at = Instant::now();
    signal(child.id(), "-INT");
    let output = child
        .wait_with_output()
        .expect("failed to wait for the child");
    let took = interrupted_at.elapsed();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(130), "stderr: {stderr}");
    assert!(
        stderr.contains("interrupted; settling"),
        "the graceful request is announced: {stderr}"
    );
    assert!(
        !stderr.contains("emergency exit"),
        "a cooperating child never needs the emergency path: {stderr}"
    );
    assert!(
        stdout.contains("\"all_passed\": false"),
        "the command finished its own report: {stdout}"
    );
    assert!(
        took < Duration::from_secs(8),
        "settled well inside the deadline, not on it: {took:?}"
    );
    assert!(process_is_gone(gate_pid), "the gate child was joined");
}

/// A second SIGINT before settlement is the distinct emergency exit: the
/// registered process groups are killed — a child ignoring TERM included —
/// and the process exits 130 at once, saying which way out it took.
#[cfg(unix)]
#[test]
fn a_second_sigint_is_the_emergency_exit_and_still_ends_a_term_ignoring_child() {
    use std::time::{Duration, Instant};
    let dir = interrupt_project("trap '' TERM HUP; echo $$ > gate.pid; sleep 30");
    let (child, gate_pid) = spawn_validate_until_gate_runs(&dir);
    let interrupted_at = Instant::now();
    signal(child.id(), "-INT");
    std::thread::sleep(Duration::from_millis(300));
    signal(child.id(), "-INT");
    let output = child
        .wait_with_output()
        .expect("failed to wait for the child");
    let took = interrupted_at.elapsed();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(130), "stderr: {stderr}");
    assert!(
        stderr.contains("emergency exit (interrupted again)"),
        "the emergency path names itself: {stderr}"
    );
    assert!(
        took < Duration::from_secs(4),
        "the emergency exit waits for no grace: {took:?}"
    );
    // The registry's kill is TERM then KILL; give the kernel a moment.
    let gone_by = Instant::now() + Duration::from_secs(3);
    while !process_is_gone(gate_pid) {
        assert!(
            Instant::now() < gone_by,
            "the TERM-ignoring gate child survived"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The bare cockpit on a real pty: the first SIGINT closes it through the
/// runtime's own leave sequence — the alternate screen left, the graceful
/// request announced in the replayed scrollback, exit 130 well inside the
/// settle deadline and never the emergency line.
#[cfg(unix)]
#[test]
fn a_sigint_to_the_cockpit_leaves_the_alternate_screen_and_exits_130_gracefully() {
    use std::io::{Read as _, Write as _};
    use std::time::{Duration, Instant};

    let dir = interrupt_project("true");
    let pty = magistr_process::Pty::open(100, 30).expect("a pty pair");
    let mut child = magistr_command::magistr_command()
        .arg("roadmap")
        .current_dir(dir.path())
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join("config"))
        .env("TERM", "xterm-256color")
        .env_remove("TMUX")
        .stdin(pty.stdio().expect("stdin on the pty"))
        .stdout(pty.stdio().expect("stdout on the pty"))
        .stderr(pty.stdio().expect("stderr on the pty"))
        .spawn()
        .expect("failed to spawn the cockpit");
    // The slave stays open until the master is drained: macOS discards a
    // slave's unread output when its last descriptor closes.
    let (master, _slave) = pty.into_parts();
    let mut master = std::fs::File::from(master);
    let mut wire = Vec::new();
    let drain = |master: &mut std::fs::File, wire: &mut Vec<u8>| {
        let mut buf = [0_u8; 4096];
        loop {
            match master.read(&mut buf) {
                Ok(0) => return,
                Ok(n) => wire.extend_from_slice(&buf[..n]),
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => return,
                Err(err) if err.raw_os_error() == Some(5) => return,
                Err(err) => panic!("reading the pty: {err}"),
            }
        }
    };
    let contains = |wire: &[u8], needle: &[u8]| wire.windows(needle.len()).any(|w| w == needle);

    // Wait for the cockpit to be on the alternate screen with a frame
    // painted — the loop is running, the startup window is over.
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut answered = false;
    loop {
        drain(&mut master, &mut wire);
        if !answered && contains(&wire, b"\x1b[c") {
            // Answer DA1 so the startup window closes without its timeout.
            master.write_all(b"\x1b[?62;4c").expect("answer the probe");
            answered = true;
        }
        if contains(&wire, b"\x1b[?1049h") && contains(&wire, b"\x1b[?2026l") {
            break;
        }
        if let Some(status) = child.try_wait().expect("poll the child") {
            panic!(
                "the cockpit exited before it was interrupted: {status}\n{}",
                String::from_utf8_lossy(&wire)
            );
        }
        assert!(
            Instant::now() < deadline,
            "the cockpit never painted a frame on the alternate screen:\n{}",
            String::from_utf8_lossy(&wire)
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let entered = wire.len();

    let interrupted_at = Instant::now();
    signal(child.id(), "-INT");
    let status = loop {
        drain(&mut master, &mut wire);
        if let Some(status) = child.try_wait().expect("poll the child") {
            break status;
        }
        assert!(
            interrupted_at.elapsed() < Duration::from_secs(20),
            "the cockpit never exited after SIGINT:\n{}",
            String::from_utf8_lossy(&wire)
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    let took = interrupted_at.elapsed();
    drain(&mut master, &mut wire);
    let text = String::from_utf8_lossy(&wire);
    assert_eq!(status.code(), Some(130), "{text}");
    assert!(
        contains(&wire[entered..], b"\x1b[?1049l"),
        "the alternate screen was left through the runtime's own sequence: {text}"
    );
    assert!(
        !text.contains("emergency exit"),
        "the graceful path never needs the emergency exit: {text}"
    );
    assert!(
        text.contains("interrupted; settling"),
        "the graceful request is announced in the replayed scrollback: {text}"
    );
    assert!(
        took < Duration::from_secs(6),
        "closed well inside the settle deadline, not on it: {took:?}"
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
