use super::*;
use crate::{
    commands::{EditInput, EditOp},
    file_content::FileContent,
    snapshot::{ChangeKind, MutationStatus, SelectionPolicy, SnapshotError, SnapshotValidation},
};

fn options(root: &Path) -> WorkspaceOpenOptions {
    WorkspaceOpenOptions::new(root, "/nonexistent/magistr-worker").with_network_override("0")
}

fn workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("workspace");
    std::fs::create_dir_all(dir.path().join(".git")).expect("repository marker");
    std::fs::write(dir.path().join("notes.txt"), "alpha\n").expect("source");
    dir
}

#[tokio::test]
async fn a_live_mutation_invalidates_an_earlier_snapshot_and_its_receipt_is_applied() {
    let dir = workspace();
    let service = WorkspaceService::open(options(dir.path()))
        .await
        .expect("owner");
    let before = service
        .source_snapshot(SelectionPolicy::default())
        .expect("capture before the check");
    assert_eq!(before.identity, service.identity());
    assert_eq!(before.identity.generation, Some(service.generation().get()));
    assert!(before.manifest.entries.contains_key("notes.txt"));
    assert!(
        service
            .validate_snapshot(&before)
            .expect("fresh")
            .is_unchanged()
    );

    let session = service.new_session().expect("session");
    let mut input = EditInput::new(EditOp::ReplaceText);
    input.file = Some("notes.txt".into());
    input.old = Some("alpha".into());
    input.new = Some("beta".into());
    input.auto_apply = true;
    let seq = match service.edit(&input, &session).await {
        WorkspaceResponse::Success { results, .. } => {
            let rows = results.rows();
            assert_eq!(rows.len(), 1);
            assert!(rows[0].applied);
            rows[0].journal_seq.expect("journaled")
        }
        WorkspaceResponse::Failure { error, notes, .. } => {
            panic!("edit refused: {} {:?}", error.message, notes)
        }
    };

    match service
        .validate_snapshot(&before)
        .expect("validate after the write")
    {
        SnapshotValidation::Changed(changes) => {
            assert_eq!(changes.len(), 1);
            assert_eq!(changes[0].path, "notes.txt");
            assert_eq!(changes[0].kind, ChangeKind::Modified);
        }
        SnapshotValidation::Unchanged => panic!("the owner's own write must be observed"),
    }
    let receipt = service
        .mutation_receipt(seq)
        .await
        .expect("read")
        .expect("exists");
    assert_eq!(receipt.status, MutationStatus::Applied);
    assert_eq!(receipt.generation, Some(service.generation().get()));
    assert_eq!(receipt.session_id, session.id());
    assert!(receipt.revision.is_some());
    assert_eq!(receipt.files[0].path, "notes.txt");
    assert_eq!(
        receipt.files[0].after_hash,
        FileContent::Text("beta\n".into()).content_hash()
    );
    let after = service
        .source_snapshot(SelectionPolicy::default())
        .expect("capture after the write");
    assert_eq!(
        after.manifest.entries["notes.txt"].digest,
        receipt.files[0].after_hash
    );
    assert_ne!(before.revision(), after.revision());
    assert!(
        service
            .mutation_receipt(seq + 100)
            .await
            .expect("read")
            .is_none()
    );
    service.shutdown().await.expect("clean shutdown");
}

#[tokio::test]
async fn the_service_refuses_a_snapshot_of_another_workspace() {
    let dir = workspace();
    let other = workspace();
    let service = WorkspaceService::open(options(dir.path()))
        .await
        .expect("owner");
    let foreign =
        crate::snapshot::SourceSnapshot::capture(other.path(), SelectionPolicy::default())
            .expect("other workspace");
    let refused = service
        .validate_snapshot(&foreign)
        .expect_err("another checkout never substitutes");
    assert!(
        matches!(refused, SnapshotError::ForeignWorkspace { .. }),
        "{refused}"
    );
    service.shutdown().await.expect("clean shutdown");
}
