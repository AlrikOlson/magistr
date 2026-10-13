use super::*;
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
fn project() -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().expect("temporary");
    let root = temp.path().join("main");
    std::fs::create_dir(&root).expect("root");
    git(&root, &["init", "-b", "main"]);
    git(&root, &["config", "user.name", "Test"]);
    git(&root, &["config", "user.email", "test@example.invalid"]);
    std::fs::write(root.join(".gitignore"), ".workbench/\n.magistr/\n").expect("ignore");
    std::fs::write(root.join("file"), "original").expect("file");
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "initial"]);
    (temp, root.canonicalize().expect("root"))
}

#[test]
fn ordinary_folders_open_without_git_and_cannot_create_git_copies() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let service = WorkingCopies::at(&root).unwrap();
    let copies = service.list().unwrap();
    assert_eq!(copies.len(), 1);
    assert_eq!(copies[0].path, root);
    assert_eq!(copies[0].state, WorkingCopyState::Unversioned);
    assert!(copies[0].is_main);
    assert_eq!(
        service.open(&root).unwrap().state,
        WorkingCopyState::Unversioned
    );
    assert!(
        service
            .create(&root.join("copy"), "copy", "HEAD")
            .unwrap_err()
            .to_string()
            .contains("does not use Git")
    );
    assert!(matches!(
        service.archive(&root),
        Err(WorkingCopyError::Main)
    ));
    assert!(!root.join(".git").exists());
}

#[test]
fn lifecycle_from_a_nested_project_still_recognizes_the_main_git_copy() {
    let (temp, main) = project();
    let nested = main.join("app");
    std::fs::create_dir(&nested).expect("nested project");
    let copies = WorkingCopies::at(&nested).expect("service");
    assert!(copies.list().expect("list")[0].is_main);
    assert!(matches!(copies.archive(&main), Err(WorkingCopyError::Main)));
    copies
        .create(&temp.path().join("task"), "task", "HEAD")
        .expect("new copy");
}

#[test]
fn create_list_archive_and_reopen_keep_files_and_history() {
    let (temp, main) = project();
    let service = WorkingCopies::at(&main).expect("service");
    let linked = temp.path().join("task");
    let created = service.create(&linked, "task", "HEAD").expect("create");
    assert_eq!(
        created.state,
        WorkingCopyState::Available {
            dirty: 0,
            conflicts: 0
        }
    );
    std::fs::write(linked.join("ignored-history"), "keep").expect("history");
    git(&linked, &["add", "ignored-history"]);
    git(&linked, &["commit", "-m", "task"]);
    let archived = service.archive(&linked).expect("archive");
    assert_eq!(archived.state, WorkingCopyState::Archived);
    assert!(
        linked.join("ignored-history").exists(),
        "archive preserves every byte"
    );
    assert!(magistr_engine::native::ProjectLocation::require_open(&linked).is_err());
    assert_eq!(service.list().expect("list").len(), 2);
    let opened = service.open(&linked).expect("reopen");
    assert!(matches!(opened.state, WorkingCopyState::Available { .. }));
    magistr_engine::native::ProjectLocation::require_open(&linked).expect("open");
}

#[test]
fn archive_refuses_active_dirty_conflicted_and_foreign_work() {
    let (temp, main) = project();
    let service = WorkingCopies::at(&main).expect("service");
    let linked = temp.path().join("task");
    service.create(&linked, "task", "HEAD").expect("create");
    let active = magistr_engine::native::WorkingCopyLease::active(&linked).expect("active");
    assert!(matches!(
        service.archive(&linked),
        Err(WorkingCopyError::Active)
    ));
    drop(active);
    std::fs::write(linked.join("file"), "changed").expect("dirty");
    assert!(matches!(
        service.archive(&linked),
        Err(WorkingCopyError::Dirty)
    ));
    assert_eq!(
        std::fs::read_to_string(linked.join("file")).expect("preserved"),
        "changed"
    );
    assert!(matches!(
        service.archive(&main),
        Err(WorkingCopyError::Main)
    ));
    assert!(matches!(
        service.open(temp.path()),
        Err(WorkingCopyError::Foreign)
    ));
    git(&linked, &["add", "file"]);
    git(&linked, &["commit", "-m", "task change"]);
    std::fs::write(main.join("file"), "different").expect("other change");
    git(&main, &["add", "file"]);
    git(&main, &["commit", "-m", "main change"]);
    let merge = Command::new("git")
        .current_dir(&linked)
        .args(["merge", "main"])
        .output()
        .expect("merge");
    assert!(!merge.status.success());
    assert!(matches!(
        service.archive(&linked),
        Err(WorkingCopyError::Conflicts)
    ));
}

#[test]
fn unavailable_working_copy_is_reported_without_pruning_or_recreation() {
    let (temp, main) = project();
    let service = WorkingCopies::at(&main).expect("service");
    let linked = temp.path().join("task");
    let linked = service
        .create(&linked, "task", "HEAD")
        .expect("create")
        .path;
    std::fs::rename(&linked, temp.path().join("moved")).expect("simulate unavailable");
    let list = service.list().expect("list");
    assert_eq!(
        list.iter()
            .find(|copy| copy.path == linked)
            .expect("record")
            .state,
        WorkingCopyState::Unavailable
    );
    assert!(matches!(
        service.open(&linked),
        Err(WorkingCopyError::Unavailable)
    ));
    assert!(!linked.exists());
}
#[test]
fn runtime_coordination_does_not_make_a_clean_copy_dirty() {
    let (temp, main) = project();
    std::fs::write(main.join(".gitignore"), "").expect("no ignored files");
    git(&main, &["add", ".gitignore"]);
    git(&main, &["commit", "-m", "no ignores"]);
    let copies = WorkingCopies::at(&main).expect("service");
    let copy = copies
        .create(&temp.path().join("task"), "task", "HEAD")
        .expect("copy");
    assert_eq!(
        copies
            .archive(&copy.path)
            .expect("archive clean source")
            .state,
        WorkingCopyState::Archived
    );
    copies.open(&copy.path).expect("reopen");
    std::fs::write(copy.path.join(".magistr/project.json"), "{}").expect("configuration");
    assert!(
        matches!(copies.archive(&copy.path), Err(WorkingCopyError::Dirty)),
        "project configuration still requires review"
    );
}
