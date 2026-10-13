use super::*;
#[test]
fn a_nested_project_keeps_its_entire_git_working_copy_active() {
    let temp = tempfile::tempdir().expect("temporary");
    std::fs::create_dir(temp.path().join(".git")).expect("repository marker");
    let nested = temp.path().join("app");
    std::fs::create_dir(&nested).expect("nested project");
    let active = WorkingCopyLease::active(&nested).expect("nested session");
    assert_eq!(
        WorkingCopyLease::archive(temp.path())
            .expect_err("whole copy is active")
            .kind(),
        io::ErrorKind::WouldBlock
    );
    drop(active);
    WorkingCopyLease::archive(temp.path()).expect("released");
}
#[test]
fn sessions_hold_shared_leases_and_archiving_needs_exclusive_access() {
    let temp = tempfile::tempdir().expect("temporary");
    let first = WorkingCopyLease::active(temp.path()).expect("first");
    let second = WorkingCopyLease::active(temp.path()).expect("second");
    assert_eq!(
        WorkingCopyLease::archive(temp.path())
            .expect_err("active")
            .kind(),
        io::ErrorKind::WouldBlock
    );
    drop(first);
    assert!(WorkingCopyLease::archive(temp.path()).is_err());
    drop(second);
    let archive = WorkingCopyLease::archive(temp.path()).expect("idle");
    assert!(WorkingCopyLease::active(temp.path()).is_err());
    drop(archive);
    WorkingCopyLease::active(temp.path()).expect("reopened");
}
