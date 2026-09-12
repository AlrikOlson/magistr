use super::*;
use crate::snapshot::{ChangeKind, SnapshotValidation};
use std::fs;

fn workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("workspace");
    let root = dir.path();
    fs::create_dir_all(root.join(".git")).expect("repository");
    fs::create_dir_all(root.join("src")).expect("src");
    fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("source");
    fs::write(root.join("blob.bin"), [0_u8, 9, 9]).expect("binary");
    fs::write(root.join(".gitignore"), "target/\n").expect("ignore");
    fs::create_dir_all(root.join("target")).expect("target");
    fs::write(root.join("target/x"), "x").expect("ignored");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::write(root.join("run.sh"), "#!/bin/sh\n").expect("script");
        fs::set_permissions(root.join("run.sh"), fs::Permissions::from_mode(0o755)).expect("mode");
        std::os::unix::fs::symlink("src/main.rs", root.join("entry")).expect("symlink");
    }
    dir
}

#[test]
fn an_export_is_immutable_and_unaffected_by_later_workspace_writes() {
    let dir = workspace();
    let optimistic = SourceSnapshot::capture(dir.path(), SelectionPolicy::default()).expect("live");
    let export = SourceExport::isolate(dir.path(), SelectionPolicy::default()).expect("export");
    assert_eq!(export.snapshot().consistency, Consistency::Immutable);
    export
        .snapshot()
        .certify_immutable()
        .expect("immutable inputs");
    assert_eq!(export.snapshot().revision(), optimistic.revision());
    assert_eq!(export.snapshot().identity, optimistic.identity);
    assert_ne!(export.root(), dir.path());
    assert_eq!(
        fs::read_to_string(export.root().join("src/main.rs")).expect("copied"),
        "fn main() {}\n"
    );
    assert_eq!(
        fs::read(export.root().join("blob.bin")).expect("binary"),
        [0_u8, 9, 9]
    );
    assert!(
        !export.root().join("target/x").exists(),
        "ignored output is not source"
    );
    assert!(!export.root().join(".git").exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(export.root().join("run.sh"))
                .expect("script")
                .permissions()
                .mode()
                & 0o7777,
            0o755
        );
        assert_eq!(
            fs::read_link(export.root().join("entry")).expect("symlink"),
            std::path::PathBuf::from("src/main.rs")
        );
    }
    // The workspace moves on; the export does not.
    fs::write(dir.path().join("src/main.rs"), "fn main() { changed() }\n").expect("edit");
    fs::remove_file(dir.path().join("blob.bin")).expect("delete");
    match export
        .snapshot()
        .validate(dir.path())
        .expect("workspace validation")
    {
        SnapshotValidation::Changed(changes) => {
            assert_eq!(
                changes
                    .iter()
                    .map(|c| (c.path.as_str(), c.kind))
                    .collect::<Vec<_>>(),
                vec![
                    ("blob.bin", ChangeKind::Removed),
                    ("src/main.rs", ChangeKind::Modified)
                ]
            );
        }
        SnapshotValidation::Unchanged => panic!("the workspace changed"),
    }
    let exported = capture::capture(
        &export.root().canonicalize().expect("canonical"),
        &export.snapshot().policy,
    )
    .expect("re-observe export");
    assert_eq!(exported, export.snapshot().manifest);
    let root = export.root().to_path_buf();
    drop(export);
    assert!(!root.exists(), "the export is removed with its handle");
}
