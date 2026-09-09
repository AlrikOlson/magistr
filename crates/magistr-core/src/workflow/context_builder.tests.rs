use super::*;
use std::fs;

/// Create a temp project with a git repo and spec file.
fn setup_project(spec_content: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::TempDir::new().unwrap();
    let magistr_dir = dir.path().join(".magistr");
    fs::create_dir_all(&magistr_dir).unwrap();

    // Create spec file
    let spec_path = dir.path().join("ROADMAP.md");
    fs::write(&spec_path, spec_content).unwrap();

    // Initialize git repo
    Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    // Initial commit
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "init: initial commit"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    (dir, spec_path)
}

#[test]
fn context_includes_spec_counts() {
    let (dir, spec_path) = setup_project("- [x] Done\n- [x] Also done\n- [ ] Todo\n");
    let builder = ContextBuilder::new(dir.path(), &spec_path);
    let context = builder.build();

    assert!(context.contains("2/3 items complete"), "context: {context}");
    assert!(context.contains("1 items"), "context: {context}");
}

#[test]
fn context_includes_git_log() {
    let (dir, spec_path) = setup_project("- [ ] Todo\n");
    let builder = ContextBuilder::new(dir.path(), &spec_path);
    let context = builder.build();

    assert!(context.contains("Recent Commits"));
    assert!(context.contains("init: initial commit"));
}

#[test]
fn context_includes_dirty_files() {
    let (dir, spec_path) = setup_project("- [ ] Todo\n");

    // Create a dirty file
    fs::write(dir.path().join("dirty.rs"), "fn main() {}").unwrap();

    let builder = ContextBuilder::new(dir.path(), &spec_path);
    let context = builder.build();

    assert!(context.contains("Uncommitted Changes"));
    assert!(context.contains("dirty.rs"));
}

#[test]
fn context_includes_active_plan() {
    let (dir, spec_path) = setup_project("- [ ] Todo\n");
    let plan_path = crate::paths::plan(dir.path());
    fs::write(&plan_path, "# My Plan\n\nDo stuff\n").unwrap();

    let builder = ContextBuilder::new(dir.path(), &spec_path);
    let context = builder.build();

    assert!(context.contains("Active Plan"));
    assert!(context.contains("Do stuff"));
}

#[test]
fn context_includes_active_task() {
    let (dir, spec_path) = setup_project("- [ ] Todo\n");
    let task_path = crate::paths::task(dir.path());
    fs::write(&task_path, "Implement widget\n").unwrap();

    let builder = ContextBuilder::new(dir.path(), &spec_path);
    let context = builder.build();

    assert!(context.contains("Active Task"));
    assert!(context.contains("Implement widget"));
}

#[test]
fn compaction_recovery_is_shorter() {
    let (dir, spec_path) = setup_project(
        "- [x] Done 1\n- [x] Done 2\n- [x] Done 3\n- [ ] Todo 1\n- [ ] Todo 2\n- [ ] Todo 3\n",
    );
    let plan_path = crate::paths::plan(dir.path());
    fs::write(
            &plan_path,
            "# Plan\n\n## Step 1\n\nDo the first thing with lots of detail.\n\n## Step 2\n\nDo the second thing with even more detail.\n\n## Step 3\n\nFinalize and verify everything is working correctly.\n",
        )
        .unwrap();
    let task_path = crate::paths::task(dir.path());
    fs::write(&task_path, "Implement context injection").unwrap();

    // Add extra dirty files to bulk up the full context
    fs::write(dir.path().join("file1.rs"), "code").unwrap();
    fs::write(dir.path().join("file2.rs"), "code").unwrap();
    fs::write(dir.path().join("file3.rs"), "code").unwrap();

    let builder = ContextBuilder::new(dir.path(), &spec_path);
    let full = builder.build();
    let recovery = builder.build_compaction_recovery();

    assert!(
        recovery.len() < full.len(),
        "recovery ({}) should be shorter than full ({})\nFull:\n{full}\nRecovery:\n{recovery}",
        recovery.len(),
        full.len()
    );
    assert!(recovery.contains("Context Recovery"));
    assert!(recovery.contains("compacted"));
}

#[test]
fn context_is_plain_text() {
    let (dir, spec_path) = setup_project("- [ ] Todo\n");
    let builder = ContextBuilder::new(dir.path(), &spec_path);
    let context = builder.build();

    // Should not contain JSON markers
    assert!(!context.contains('{'));
    assert!(!context.contains('}'));
    // Should be markdown/plain text
    assert!(context.contains('#'));
}

#[test]
fn works_without_git_repo() {
    let dir = tempfile::TempDir::new().unwrap();
    let magistr_dir = dir.path().join(".magistr");
    fs::create_dir_all(&magistr_dir).unwrap();
    let spec_path = dir.path().join("ROADMAP.md");
    fs::write(&spec_path, "- [ ] Todo\n").unwrap();

    let builder = ContextBuilder::new(dir.path(), &spec_path);
    let context = builder.build();

    // Should still include spec info
    assert!(context.contains("Remaining:"), "context: {context}");
    assert!(context.contains("1 items"), "context: {context}");
    // Should not crash, just skip git sections
    assert!(!context.contains("Recent Commits"));
}

#[test]
fn max_commits_limits_output() {
    let (dir, spec_path) = setup_project("- [ ] Todo\n");

    // Create additional commits
    for i in 1..=5 {
        fs::write(
            dir.path().join(format!("file{i}.txt")),
            format!("content {i}"),
        )
        .unwrap();
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", &format!("commit {i}")])
            .current_dir(dir.path())
            .output()
            .unwrap();
    }

    let builder = ContextBuilder::new(dir.path(), &spec_path).max_commits(2);
    let ctx = builder.gather();

    assert_eq!(ctx.recent_commits.len(), 2);
}

#[test]
fn gather_populates_all_fields() {
    let (dir, spec_path) = setup_project("- [x] Done\n- [ ] Remaining\n");
    let builder = ContextBuilder::new(dir.path(), &spec_path);
    let ctx = builder.gather();

    // Commits from setup
    assert!(!ctx.recent_commits.is_empty());
    // No dirty files after clean commit
    assert!(ctx.dirty_files.is_empty());
    // No active plan/task by default
    assert!(ctx.active_plan.is_none());
    assert!(ctx.active_task.is_none());
}

#[test]
fn recovery_includes_last_commit_only() {
    let (dir, spec_path) = setup_project("- [ ] Todo\n");

    // Add a second commit
    fs::write(dir.path().join("file.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "feat: second commit"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let builder = ContextBuilder::new(dir.path(), &spec_path);
    let recovery = builder.build_compaction_recovery();

    assert!(recovery.contains("second commit"));
    // Should say "Last commit" not "Recent Commits"
    assert!(recovery.contains("Last commit"));
    assert!(!recovery.contains("Recent Commits"));
}

#[test]
fn recovery_includes_dirty_file_count() {
    let (dir, spec_path) = setup_project("- [ ] Todo\n");
    fs::write(dir.path().join("a.rs"), "a").unwrap();
    fs::write(dir.path().join("b.rs"), "b").unwrap();

    let builder = ContextBuilder::new(dir.path(), &spec_path);
    let recovery = builder.build_compaction_recovery();

    assert!(recovery.contains("Uncommitted files: 2"));
}
