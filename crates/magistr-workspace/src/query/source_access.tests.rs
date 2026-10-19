use super::*;
use crate::{
    config::ResolveOptions,
    file_content::FileContent,
    worker::{WorkerOptions, WorkerSupervisor},
    workspace_policy::WorkspacePolicy,
};
use std::fs;

fn worker() -> WorkerSupervisor {
    WorkerSupervisor::new(
        std::env::current_exe().expect("test executable"),
        WorkerOptions::default(),
    )
    .expect("supervisor")
}

#[test]
fn snapshot_preserves_original_bytes_hash_and_rejects_directories() {
    let dir = tempfile::tempdir().expect("fixture");
    fs::write(dir.path().join("file.txt"), "first\r\n😀\n").expect("source");
    let policy = WorkspacePolicy::resolve(&ResolveOptions::new(dir.path())).expect("policy");
    let worker = worker();
    let query = WorkspaceQueries::new(&policy, &worker, None).expect("queries");
    let first = query.snapshot("file.txt").expect("snapshot");
    assert_eq!(first.content.as_bytes(), b"first\r\n\xf0\x9f\x98\x80\n");
    assert_eq!(
        first.content_hash,
        FileContent::Text("first\r\n😀\n".into()).content_hash()
    );
    assert!(query.snapshot(".").is_err());
    fs::write(dir.path().join("file.txt"), "changed").expect("change");
    assert_ne!(
        first.content_hash,
        query
            .snapshot("file.txt")
            .expect("fresh snapshot")
            .content_hash
    );
}

#[test]
fn oversized_source_is_a_typed_observation_and_a_named_refusal() {
    let dir = tempfile::tempdir().expect("fixture");
    fs::create_dir(dir.path().join("data")).expect("directory");
    let huge = vec![b'x'; MAX_FILE_BYTES + 1];
    fs::write(dir.path().join("data/huge.json"), &huge).expect("oversized source");
    fs::write(dir.path().join("data/small.json"), b"{}").expect("small source");
    let policy = WorkspacePolicy::resolve(&ResolveOptions::new(dir.path())).expect("policy");
    let worker = worker();
    let query = WorkspaceQueries::new(&policy, &worker, None).expect("queries");
    match query.observe("data/huge.json").expect("observation") {
        SourceRead::Oversized { bytes } => assert_eq!(bytes, huge.len() as u64),
        SourceRead::Observed(_) => panic!("an oversized file is never observed as bytes"),
    }
    assert!(matches!(
        query.observe("data/small.json").expect("observation"),
        SourceRead::Observed(_)
    ));
    let refusal = query
        .snapshot("data\\huge.json")
        .expect_err("named refusal");
    assert_eq!(refusal.kind, ErrorKind::BudgetTooSmall);
    assert!(
        refusal.message.contains("data/huge.json")
            && refusal.message.contains("1 MiB")
            && refusal.message.contains(&huge.len().to_string()),
        "{}",
        refusal.message
    );
}

#[test]
fn live_walk_honors_nested_ignores_metadata_and_empty_directories() {
    let dir = tempfile::tempdir().expect("fixture");
    for path in ["src/empty", ".git", ".magistr/workspace", "vendor"] {
        fs::create_dir_all(dir.path().join(path)).expect("directory");
    }
    for path in [
        "src/yes.rs",
        "src/no.rs",
        ".git/secret",
        ".magistr/workspace/state",
        "vendor/file.rs",
    ] {
        fs::write(dir.path().join(path), "").expect("file");
    }
    fs::write(dir.path().join("src/.ignore"), "no.rs\n").expect("ignore");
    let policy = WorkspacePolicy::resolve(&ResolveOptions::new(dir.path())).expect("policy");
    let worker = worker();
    let query = WorkspaceQueries::new(&policy, &worker, None).expect("queries");
    let rows = query.files().expect("source walk");
    assert!(rows.iter().any(|row| row.path == "src/empty" && row.is_dir));
    assert!(
        rows.iter()
            .any(|row| row.path == "src/yes.rs" && !row.is_dir)
    );
    assert!(
        rows.iter()
            .any(|row| row.path == "vendor/file.rs" && row.junk)
    );
    assert!(!rows.iter().any(|row| row.path == "src/no.rs"
        || row.path.starts_with(".git")
        || row.path.starts_with(".magistr/workspace")));
}

#[cfg(unix)]
#[test]
fn snapshot_refuses_outside_symlink_and_nonregular_file() {
    let dir = tempfile::tempdir().expect("fixture");
    let outside = tempfile::tempdir().expect("outside");
    fs::write(outside.path().join("secret"), "secret").expect("outside source");
    std::os::unix::fs::symlink(outside.path().join("secret"), dir.path().join("escape"))
        .expect("link");
    let policy = WorkspacePolicy::resolve(&ResolveOptions::new(dir.path())).expect("policy");
    let worker = worker();
    let query = WorkspaceQueries::new(&policy, &worker, None).expect("queries");
    assert_eq!(
        query.snapshot("escape").expect_err("refusal").code(),
        "PATH_OUTSIDE_WORKSPACE"
    );
}
