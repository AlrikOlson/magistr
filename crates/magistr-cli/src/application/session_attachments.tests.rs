use super::*;
use crate::application::{
    session_observation::SessionObservation,
    session_registry::tests::{Log, projects, registry},
};
use magistr_application::sessions::{AgentEvent, MAX_ATTACHMENT_BYTES};
use magistr_cockpit::cockpit::Msg;
use magistr_cockpit_runtime::channel::ObservationSink;
use magistr_domain::identity::SessionAddress;
use magistr_workspace::service::WorkspaceOpenOptions;
use std::sync::Arc;

fn address() -> SessionAddress {
    SessionAddress {
        project: "test".into(),
        workspace: "/test".into(),
        session: "one".into(),
    }
}

fn pasted(name: &str, bytes: &[u8]) -> Vec<StagedAttachment> {
    AttachmentSelection::Bytes {
        name: name.into(),
        bytes: bytes.to_vec(),
    }
    .prepare(None)
    .unwrap()
}

#[tokio::test]
async fn selected_file_changes_refuse_send_and_accepted_bytes_do_not_change() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notes.txt");
    std::fs::write(&path, "日本語\r\n").unwrap();
    let service = WorkspaceService::open(
        WorkspaceOpenOptions::new(dir.path(), "/nonexistent/attachment-worker")
            .with_network_override("0"),
    )
    .await
    .unwrap();
    let selected = AttachmentSelection::Files(vec![path.clone()])
        .prepare(Some(&service))
        .unwrap();
    let input = prepare_send("review".into(), &selected, Some(&service)).unwrap();
    assert_eq!(input.attachments[0].bytes(), "日本語\r\n".as_bytes());
    assert_eq!(selected[0].path(), Some("notes.txt"));
    std::fs::write(&path, "changed").unwrap();
    assert!(
        prepare_send("review".into(), &selected, Some(&service))
            .unwrap_err()
            .contains("changed")
    );
    assert_eq!(input.attachments[0].bytes(), "日本語\r\n".as_bytes());
    std::fs::remove_file(&path).unwrap();
    assert!(
        prepare_send("review".into(), &selected, Some(&service))
            .unwrap_err()
            .contains("missing")
    );
    service.shutdown().await.unwrap();
}

#[test]
fn source_and_clipboard_refusals_preserve_existing_draft_and_attachments() {
    let owner = SessionObservation::new(address());
    owner.draft("keep this".into());
    owner
        .add_attachments(pasted("notes.txt", b"first"))
        .unwrap();
    let before = owner.snapshot();
    assert!(
        AttachmentSelection::Files(vec![PathBuf::from("missing")])
            .prepare(None)
            .is_err()
    );
    assert!(
        AttachmentSelection::Bytes {
            name: "huge".into(),
            bytes: vec![b'x'; MAX_ATTACHMENT_BYTES + 1]
        }
        .prepare(None)
        .is_err()
    );
    assert!(
        AttachmentSelection::Bytes {
            name: "binary".into(),
            bytes: vec![0, 255]
        }
        .prepare(None)
        .is_err()
    );
    assert_eq!(owner.snapshot(), before);
    let more = (0..MAX_ATTACHMENTS)
        .flat_map(|n| pasted(&format!("{n}.txt"), b"x"))
        .collect();
    assert!(owner.add_attachments(more).unwrap_err().contains("up to"));
    assert_eq!(owner.snapshot(), before);
}

#[test]
fn acceptance_compares_the_attached_bytes_and_rejection_restores_exact_content() {
    let owner = SessionObservation::new(address());
    let old = pasted("notes.txt", b"old");
    let new = pasted("notes.txt", b"new");
    let old_input = prepare_send("same text".into(), &old, None).unwrap();
    owner.draft("same text".into());
    owner.add_attachments(new.clone()).unwrap();
    owner.send(Msg::InputAccepted(old_input.receipt()));
    assert_eq!(owner.snapshot().attachments, new);
    assert_eq!(owner.snapshot().seed.draft.as_deref(), Some("same text"));
    let new_input = prepare_send("same text".into(), &new, None).unwrap();
    owner.send(Msg::InputAccepted(new_input.receipt()));
    assert!(owner.snapshot().attachments.is_empty());
    assert_eq!(owner.snapshot().seed.draft.as_deref(), Some(""));
    owner.send(Msg::Agent(AgentEvent::PromptRejected {
        input: new_input.clone(),
        error: "connection lost".into(),
    }));
    let restored = owner.snapshot();
    assert!(matches_input(&restored.attachments, &new_input));
    owner.send(Msg::Agent(AgentEvent::PromptRejected {
        input: old_input,
        error: "late".into(),
    }));
    assert_eq!(owner.snapshot().attachments, restored.attachments);
}

#[tokio::test]
async fn addressed_pending_files_survive_switching_refusal_and_cancellation() {
    let tmp = projects();
    let registry = registry(Log::default());
    let client = registry.client();
    let first = client
        .open(
            tmp.path().join("first"),
            crate::application::SessionOpening::Launch,
        )
        .await
        .unwrap();
    let second = client
        .open(
            tmp.path().join("second"),
            crate::application::SessionOpening::Launch,
        )
        .await
        .unwrap();
    client.draft(&first, 0, "image draft".into()).await.unwrap();
    client
        .attach_bytes(
            &first,
            "picture.png".into(),
            b"\x89PNG\r\n\x1a\nimage".to_vec(),
        )
        .await
        .unwrap();
    let mut a = client.subscribe(&first).await.unwrap();
    let mut b = client.subscribe(&second).await.unwrap();
    assert!(b.snapshot().attachments.is_empty());
    let staged = a.snapshot().attachments;
    let ids = staged.iter().map(|file| file.id().to_owned()).collect();
    assert!(
        client
            .send(&first, "image draft".into(), ids)
            .await
            .unwrap_err()
            .contains("does not accept images")
    );
    client
        .command(&first, magistr_cockpit::cockpit::Cmd::Cancel)
        .await
        .unwrap();
    assert_eq!(a.snapshot().attachments, staged);
    assert_eq!(a.snapshot().seed.draft.as_deref(), Some("image draft"));
    assert!(
        client
            .remove_attachment(&second, staged[0].id().into())
            .await
            .is_err()
    );
    client
        .remove_attachment(&first, staged[0].id().into())
        .await
        .unwrap();
    assert!(
        client
            .subscribe(&first)
            .await
            .unwrap()
            .snapshot()
            .attachments
            .is_empty()
    );
    assert!(
        client
            .send(&first, "image draft".into(), vec![staged[0].id().into()])
            .await
            .unwrap_err()
            .contains("changed")
    );
    client.close(&first).await.unwrap();
    assert!(
        client
            .attach_bytes(&first, "later.txt".into(), b"late".to_vec())
            .await
            .is_err()
    );
    registry.shutdown().await.unwrap();
}

#[test]
fn acceptance_of_removed_content_does_not_clear_an_identical_new_selection() {
    let owner = SessionObservation::new(address());
    owner.draft("same text".into());
    owner
        .add_attachments(pasted("notes.txt", b"same bytes"))
        .unwrap();
    let selected = owner.snapshot().attachments;
    let old_input = prepare_send("same text".into(), &selected, None).unwrap();
    owner.remove_attachment(selected[0].id()).unwrap();
    owner
        .add_attachments(pasted("notes.txt", b"same bytes"))
        .unwrap();
    let replacement = owner.snapshot().attachments;
    assert_ne!(selected[0].id(), replacement[0].id());
    assert_eq!(
        selected[0].content().bytes(),
        replacement[0].content().bytes()
    );
    owner.send(Msg::InputAccepted(old_input.receipt()));
    assert_eq!(owner.snapshot().attachments, replacement);
    assert_eq!(owner.snapshot().seed.draft.as_deref(), Some("same text"));
    let current = prepare_send("same text".into(), &replacement, None).unwrap();
    owner.send(Msg::InputAccepted(current.receipt()));
    assert!(owner.snapshot().attachments.is_empty());
}
#[tokio::test]
async fn source_reader_lifetime_does_not_keep_an_admitted_owner_alive() {
    let dir = tempfile::tempdir().unwrap();
    let source = Arc::new(
        WorkspaceService::open(
            WorkspaceOpenOptions::new(dir.path(), "/nonexistent/attachment-worker")
                .with_network_override("0"),
        )
        .await
        .unwrap(),
    );
    let owner = SessionObservation::new(address());
    owner.set_source(Arc::downgrade(&source));
    assert!(owner.source().is_some());
    let service = Arc::try_unwrap(source)
        .ok()
        .expect("the observer must not retain its file owner");
    service.shutdown().await.unwrap();
    assert!(owner.source().is_none());
}
