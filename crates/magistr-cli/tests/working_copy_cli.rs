//! The public working-copy commands use the application's Git lifecycle.
#[path = "support/magistr_command.rs"]
mod magistr_command;

use serde_json::Value;
use std::path::Path;
use std::process::Command;

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .expect("git");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run(root: &Path, action: &str, path: Option<&Path>) -> std::process::Output {
    let mut command = magistr_command::magistr_command();
    command
        .current_dir(root)
        .args(["workspace", "working-copy", "--dir", "."])
        .arg(action);
    if let Some(path) = path {
        command.arg(path);
    }
    if action == "create" {
        command.args(["--branch", "task"]);
    }
    command.output().expect("CLI")
}

fn value(output: std::process::Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("working-copy JSON")
}

#[test]
fn commands_create_list_archive_and_reopen_without_discarding_changes() {
    let temp = tempfile::tempdir().expect("temporary");
    let root = temp.path().join("main");
    std::fs::create_dir(&root).expect("main");
    git(&root, &["init", "-b", "main"]);
    git(&root, &["config", "user.name", "Test"]);
    git(&root, &["config", "user.email", "test@example.invalid"]);
    std::fs::write(root.join("file"), "original").expect("source");
    git(&root, &["add", "file"]);
    git(
        &root,
        &["-c", "commit.gpgsign=false", "commit", "-m", "initial"],
    );
    let linked = temp.path().join("task");
    let created = value(run(&root, "create", Some(&linked)));
    assert_eq!(created["branch"], "task");
    assert_eq!(created["state"]["status"], "available");
    assert_eq!(
        value(run(&root, "list", None))
            .as_array()
            .expect("copies")
            .len(),
        2
    );
    assert_eq!(
        value(run(&root, "archive", Some(&linked)))["state"]["status"],
        "archived"
    );
    assert_eq!(
        std::fs::read_to_string(linked.join("file")).expect("kept source"),
        "original"
    );
    assert_eq!(
        value(run(&root, "open", Some(&linked)))["state"]["status"],
        "available"
    );
    std::fs::write(linked.join("file"), "unreviewed").expect("local change");
    let refused = run(&root, "archive", Some(&linked));
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("local changes"));
    assert_eq!(
        std::fs::read_to_string(linked.join("file")).expect("kept changes"),
        "unreviewed"
    );
}
