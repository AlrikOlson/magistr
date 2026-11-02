//! E2e: the THINK engine, wired with a `RepoSink`, mirrors each
//! recorded note into the repo's `.magistr/traces/` as Agent Trace JSONL and
//! commits the log on close. Exercises the real engine + real git.

use std::path::Path;
use std::process::Command;

use magistr_workflow::decision::config::DecisionConfig;
use magistr_workflow::decision::domain::Note;
use magistr_workflow::decision::engine::core::ReasoningServer;
use magistr_workflow::infra::RepoSink;
use serde_json::{Value, json};
use tempfile::TempDir;

fn git(repo: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap()
}

fn init_repo(repo: &Path) {
    assert!(git(repo, &["init", "-q"]).status.success());
    git(repo, &["config", "user.email", "test@example.com"]);
    git(repo, &["config", "user.name", "Test"]);
    git(repo, &["config", "commit.gpgsign", "false"]);
}

fn quiet_config() -> DecisionConfig {
    let mut c = DecisionConfig::default();
    c.display.color_output = false;
    c
}

/// A minimal valid note, built via serde so we don't restate ~20 fields.
fn note(n: u32, final_note: bool) -> Note {
    let mut v = json!({
        "note": n,
        "estimated_total": 3,
        "purpose": "analysis",
        "context": "ctx",
        "thought": "thought",
        "outcome": "outcome",
        "next_action": "next",
        "rationale": "rationale",
    });
    if final_note {
        v["is_final_note"] = json!(true);
    }
    serde_json::from_value(v).expect("valid note json")
}

fn jsonl_files(dir: &Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().map(|x| x == "jsonl").unwrap_or(false))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn shared_notes_mirror_as_agent_log_and_commit_on_close() {
    let tmp = TempDir::new().unwrap();
    init_repo(tmp.path());

    let mut server = ReasoningServer::new(quiet_config()).with_repo_sink(
        RepoSink::new(tmp.path()),
        true, // shared → committed logs/ partition
    );

    assert!(server.process_note_native(note(1, false)).is_ok());
    assert!(server.process_note_native(note(2, false)).is_ok());
    assert!(server.process_note_native(note(3, true)).is_ok()); // closes the log
    // The mirror is asynchronous; wait for the worker to drain before asserting.
    server.flush_mirror();

    // Exactly one log file with three valid Agent Trace records.
    let logs = tmp.path().join(".magistr/traces/sessions");
    let files = jsonl_files(&logs);
    assert_eq!(files.len(), 1, "one log JSONL file: {files:?}");
    let body = std::fs::read_to_string(&files[0]).unwrap();
    assert_eq!(body.lines().count(), 3, "one record per note");
    for line in body.lines() {
        let v: Value = serde_json::from_str(line).expect("each line is valid JSON");
        assert_eq!(v["version"], "0.1.0");
        assert_eq!(v["tool"]["name"], "magistr");
        assert_eq!(v["vcs"]["type"], "git");
        let ext = &v["metadata"]["dev.magistr"];
        assert_eq!(ext["family"], "decision");
        assert_eq!(ext["kind"], "note");
        assert_eq!(ext["shared"], true);
        assert!(ext["record"]["note"].is_number());
    }

    // The close (is_final_note on note 3) produced exactly one commit.
    let log = git(tmp.path(), &["log", "--oneline", "--", ".magistr/traces/"]);
    let commits = String::from_utf8_lossy(&log.stdout);
    assert_eq!(commits.lines().count(), 1, "one commit per log: {commits}");

    // The committed file is tracked; the partition gitignore excludes local/.
    let tracked = git(tmp.path(), &["ls-files", ".magistr/traces/"]);
    let tracked = String::from_utf8_lossy(&tracked.stdout);
    assert!(tracked.contains("sessions/"), "log file tracked: {tracked}");
}

#[test]
fn local_default_writes_but_never_commits() {
    let tmp = TempDir::new().unwrap();
    init_repo(tmp.path());

    // shared = false → records land in the gitignored local/ partition.
    let mut server =
        ReasoningServer::new(quiet_config()).with_repo_sink(RepoSink::new(tmp.path()), false);
    server.process_note_native(note(1, true)).unwrap();
    server.flush_mirror();

    let local = tmp.path().join(".magistr/traces/local");
    assert_eq!(jsonl_files(&local).len(), 1, "local file written");
    assert!(
        !tmp.path().join(".magistr/traces/sessions").exists()
            || jsonl_files(&tmp.path().join(".magistr/traces/sessions")).is_empty(),
        "nothing in the shared partition"
    );

    // No commits: local is gitignored and commit_log only runs for shared.
    let log = git(tmp.path(), &["log", "--oneline"]);
    let empty = !log.status.success() || String::from_utf8_lossy(&log.stdout).trim().is_empty();
    assert!(empty, "local-only logs must not be committed");
}

#[test]
fn no_sink_preserves_local_behaviour() {
    // Without a sink, the engine works exactly as before and touches no repo.
    let mut server = ReasoningServer::new(quiet_config());
    assert!(server.process_note_native(note(1, true)).is_ok());
}
