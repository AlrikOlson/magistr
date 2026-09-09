use super::*;
use tempfile::TempDir;

fn init_git_repo(dir: &Path) {
    Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .output()
        .expect("git init");
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir)
        .output()
        .expect("git config email");
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir)
        .output()
        .expect("git config name");
    // Create initial commit so stash works
    std::fs::write(dir.join("README.md"), "# test\n").expect("write readme");
    Command::new("git")
        .args(["add", "."])
        .current_dir(dir)
        .output()
        .expect("git add");
    Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(dir)
        .output()
        .expect("git commit");
}

#[test]
fn rollback_not_needed_when_baseline_failing() {
    let dir = TempDir::new().unwrap();
    let mut guard = RollbackGuard::new(dir.path(), false);
    let result = guard.check_and_rollback(false);
    assert_eq!(result, RollbackResult::NotNeeded);
}

#[test]
fn rollback_not_needed_when_still_passing() {
    let dir = TempDir::new().unwrap();
    let mut guard = RollbackGuard::new(dir.path(), true);
    let result = guard.check_and_rollback(true);
    assert_eq!(result, RollbackResult::NotNeeded);
}

#[test]
fn rollback_not_needed_when_no_dirty_files() {
    let dir = TempDir::new().unwrap();
    init_git_repo(dir.path());
    let mut guard = RollbackGuard::new(dir.path(), true);
    let result = guard.check_and_rollback(false);
    assert_eq!(result, RollbackResult::NotNeeded);
}

#[test]
fn rollback_stashes_dirty_files() {
    let dir = TempDir::new().unwrap();
    init_git_repo(dir.path());

    // Create a dirty file
    std::fs::write(dir.path().join("broken.rs"), "fn bad() {}").expect("write file");

    let mut guard = RollbackGuard::new(dir.path(), true);
    let result = guard.check_and_rollback(false);

    match result {
        RollbackResult::Stashed {
            stash_ref,
            files_affected,
            failure_reason,
        } => {
            assert_eq!(stash_ref, "stash@{0}");
            assert_eq!(files_affected, 1);
            assert!(failure_reason.contains("passing to failing"));
        }
        other => panic!("Expected Stashed, got: {other:?}"),
    }

    // File should be gone from working tree
    assert!(!dir.path().join("broken.rs").exists());
    assert!(guard.is_rolled_back());
}

#[test]
fn rollback_only_happens_once() {
    let dir = TempDir::new().unwrap();
    init_git_repo(dir.path());

    std::fs::write(dir.path().join("broken.rs"), "fn bad() {}").expect("write file");

    let mut guard = RollbackGuard::new(dir.path(), true);
    let first = guard.check_and_rollback(false);
    assert!(matches!(first, RollbackResult::Stashed { .. }));

    // Create another dirty file
    std::fs::write(dir.path().join("broken2.rs"), "fn bad2() {}").expect("write file");

    // Second rollback should be NotNeeded since already rolled back
    let second = guard.check_and_rollback(false);
    assert_eq!(second, RollbackResult::NotNeeded);
}

#[test]
fn count_dirty_files_works() {
    let dir = TempDir::new().unwrap();
    init_git_repo(dir.path());

    assert_eq!(count_dirty_files(dir.path()), 0);

    std::fs::write(dir.path().join("a.txt"), "a").expect("write");
    std::fs::write(dir.path().join("b.txt"), "b").expect("write");
    assert_eq!(count_dirty_files(dir.path()), 2);
}

#[test]
fn git_head_returns_sha() {
    let dir = TempDir::new().unwrap();
    init_git_repo(dir.path());

    let head = git_head(dir.path());
    assert!(head.is_some());
    assert!(!head.unwrap().is_empty());
}

#[test]
fn git_head_returns_none_for_non_repo() {
    let dir = TempDir::new().unwrap();
    let head = git_head(dir.path());
    assert!(head.is_none());
}

#[test]
fn baseline_passing_accessor() {
    let dir = TempDir::new().unwrap();
    let guard = RollbackGuard::new(dir.path(), true);
    assert!(guard.baseline_passing());

    let guard2 = RollbackGuard::new(dir.path(), false);
    assert!(!guard2.baseline_passing());
}
