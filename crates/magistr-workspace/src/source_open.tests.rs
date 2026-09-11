use super::*;
use std::{fs, io::Read, os::unix::fs::symlink};

#[test]
fn raced_fifo_opens_without_blocking_and_is_not_regular_source() {
    use std::{ffi::CString, os::unix::ffi::OsStrExt, time::Duration};
    let directory = tempfile::tempdir().expect("workspace");
    let root = directory.path().canonicalize().expect("canonical root");
    let fifo = root.join("source.rs");
    let name = CString::new(fifo.as_os_str().as_bytes()).expect("FIFO path");
    // SAFETY: name is a valid NUL-terminated path; mode is a POSIX permission mask.
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    let (sender, receiver) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        let opened = open_source(&root, &fifo);
        sender.send(opened).expect("receiver");
    });
    let file = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("FIFO admission must not wait for a writer")
        .expect("nonblocking descriptor");
    assert!(!file.metadata().expect("FIFO metadata").is_file());
    reader.join().expect("reader joins");
}

#[test]
fn descriptor_walk_reads_nested_unicode_source() {
    let directory = tempfile::tempdir().expect("workspace");
    let root = directory.path().canonicalize().expect("canonical root");
    fs::create_dir_all(root.join("nested/文字")).expect("directories");
    fs::write(root.join("nested/文字/source.rs"), "fn safe() {}\n").expect("source");
    let mut file =
        open_source(&root, &root.join("nested/文字/source.rs")).expect("descriptor open");
    let mut content = String::new();
    file.read_to_string(&mut content).expect("read source");
    assert_eq!(content, "fn safe() {}\n");
}

#[test]
fn descriptor_walk_rejects_intermediate_link_installed_after_admission() {
    let directory = tempfile::tempdir().expect("workspace");
    let root = directory.path().canonicalize().expect("canonical root");
    let outside = tempfile::tempdir().expect("outside");
    fs::create_dir(root.join("nested")).expect("directory");
    fs::write(root.join("nested/source.rs"), "inside").expect("source");
    fs::write(outside.path().join("source.rs"), "outside secret").expect("outside source");
    let admitted = root
        .join("nested/source.rs")
        .canonicalize()
        .expect("admission before replacement");
    fs::rename(root.join("nested"), root.join("previous")).expect("replace intermediate directory");
    symlink(outside.path(), root.join("nested")).expect("raced ancestor link");
    assert_eq!(
        fs::read_to_string(&admitted).expect("ordinary path follows replacement"),
        "outside secret"
    );
    assert!(
        open_source(&root, &admitted).is_err(),
        "an outside file must never be opened through an intermediate link"
    );
}

#[test]
fn descriptor_walk_rejects_final_link_and_replaced_workspace_ancestor() {
    let directory = tempfile::tempdir().expect("parent");
    let parent = directory.path().canonicalize().expect("canonical parent");
    let root = parent.join("workspace");
    fs::create_dir(&root).expect("workspace");
    fs::write(root.join("source.rs"), "inside").expect("source");
    symlink(root.join("source.rs"), root.join("alias.rs")).expect("final link");
    assert!(open_source(&root, &root.join("alias.rs")).is_err());
    fs::rename(&root, parent.join("original")).expect("replace workspace");
    symlink(parent.join("original"), &root).expect("raced workspace link");
    assert!(open_source(&root, &root.join("source.rs")).is_err());
}
