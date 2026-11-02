//! E2e: the RUN engine, wired with a `RepoSink`, mirrors each
//! mutation into the repo's `.magistr/traces/` as Agent Trace JSONL and commits
//! the session on `run` (goal shipped). Exercises the real engine + git.

use std::path::Path;
use std::process::Command;

use magistr_workflow::infra::RepoSink;
use magistr_workflow::run::domain::check::CheckType;
use magistr_workflow::run::domain::progress::ProgressType;
use magistr_workflow::run::domain::step::StepType;
use magistr_workflow::run::engine::RunEngine;
use serde_json::Value;
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
fn run_lifecycle_mirrors_records_and_commits_on_run() {
    let tmp = TempDir::new().unwrap();
    init_repo(tmp.path());

    let mut engine = RunEngine::new("proj-abc".to_string())
        .with_repo_sink(RepoSink::new(tmp.path()), true /* shared */);

    // Full lifecycle: goal → plan → start → record(code) → check → run.
    engine
        .set_goal(
            "Build it".into(),
            vec!["works".into()],
            vec![],
            "src/".into(),
        )
        .unwrap();
    engine
        .add_step(
            "impl".into(),
            "Implement".into(),
            StepType::Implement,
            None,
            None,
        )
        .unwrap();
    engine.start_step("impl").unwrap();
    engine
        .record_progress(
            Some("impl"),
            ProgressType::Code,
            "edit lib.rs".into(),
            vec!["src/lib.rs".into()],
            vec!["Edit".into()],
            "done".into(),
            None,
        )
        .unwrap();
    engine
        .record_check(
            Some("impl"),
            CheckType::Test,
            "cargo test".into(),
            true,
            "all pass".into(),
            true,
        )
        .unwrap();
    serde_json::to_value(engine.finalize(vec![], Some("done".into())).unwrap().value).unwrap(); // closes the session
    // The mirror is asynchronous; wait for the worker to drain before asserting.
    engine.flush_mirror();

    // One session file with the full set of kinds, in order.
    let sessions = tmp.path().join(".magistr/traces/sessions");
    let files = jsonl_files(&sessions);
    assert_eq!(files.len(), 1, "one run session file: {files:?}");
    let body = std::fs::read_to_string(&files[0]).unwrap();
    let recs: Vec<Value> = body
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();

    let kinds: Vec<&str> = recs
        .iter()
        .map(|r| r["metadata"]["dev.magistr"]["kind"].as_str().unwrap())
        .collect();
    // goal (set), step (added), step (started), progress, check, goal (shipped)
    assert_eq!(
        kinds,
        vec!["goal", "step", "step", "progress", "check", "goal"]
    );

    for r in &recs {
        assert_eq!(r["version"], "0.1.0");
        assert_eq!(r["metadata"]["dev.magistr"]["family"], "run");
        assert_eq!(r["tool"]["name"], "magistr");
    }

    // The code progress carries Agent Trace files[] attribution.
    let progress = recs
        .iter()
        .find(|r| r["metadata"]["dev.magistr"]["kind"] == "progress")
        .unwrap();
    assert_eq!(progress["files"][0]["path"], "src/lib.rs");
    assert_eq!(
        progress["files"][0]["conversations"][0]["contributor"]["type"],
        "ai"
    );

    // Exactly one commit, produced on run().
    let log = git(tmp.path(), &["log", "--oneline", "--", ".magistr/traces/"]);
    let commits = String::from_utf8_lossy(&log.stdout);
    assert_eq!(
        commits.lines().count(),
        1,
        "one commit per session: {commits}"
    );
}

#[test]
fn run_local_default_writes_but_never_commits() {
    let tmp = TempDir::new().unwrap();
    init_repo(tmp.path());

    let mut engine =
        RunEngine::new("proj-abc".to_string()).with_repo_sink(RepoSink::new(tmp.path()), false);
    engine
        .set_goal("g".into(), vec![], vec![], String::new())
        .unwrap();
    serde_json::to_value(engine.finalize(vec![], None).unwrap().value).unwrap();
    engine.flush_mirror();

    let local = tmp.path().join(".magistr/traces/local");
    assert_eq!(jsonl_files(&local).len(), 1, "local file written");
    let log = git(tmp.path(), &["log", "--oneline"]);
    let empty = !log.status.success() || String::from_utf8_lossy(&log.stdout).trim().is_empty();
    assert!(empty, "local-only run traces must not be committed");
}

#[test]
fn run_no_sink_preserves_behaviour() {
    let mut engine = RunEngine::new("proj-abc".to_string());
    engine
        .set_goal("g".into(), vec![], vec![], String::new())
        .unwrap();
    engine
        .add_step("t".into(), "T".into(), StepType::Implement, None, None)
        .unwrap();
    // No panic, no repo touched — works exactly as before.
    let report = serde_json::to_value(engine.finalize(vec![], None).unwrap().value).unwrap();
    assert!(report.is_object());
}
