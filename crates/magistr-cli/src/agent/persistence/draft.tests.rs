use super::*;
use crate::agent::persistence::{
    SessionHistory, session_filter::SessionFilter, session_outcome::SessionOutcome,
};
use chrono::Utc;

fn pasted(name: &str, bytes: &[u8]) -> Vec<StagedAttachment> {
    let hash = FileContent::decode(bytes.to_vec()).content_hash();
    vec![StagedAttachment::new(
        PromptAttachment::from_bytes(name, hash, bytes.to_vec()).unwrap(),
        None,
    )]
}

fn inputs(dir: &tempfile::TempDir) -> ProjectInputs {
    ProjectInputs::from_lookup(dir.path(), |key| {
        (key == "XDG_DATA_HOME").then(|| dir.path().join("state").display().to_string())
    })
}

async fn history(dir: &tempfile::TempDir) -> (ProjectInputs, SessionHistory, String) {
    let inputs = inputs(dir);
    let history = SessionHistory::new();
    history
        .begin(
            &inputs,
            dir.path(),
            "unavailable-agent",
            Some("chosen-model"),
        )
        .await
        .unwrap();
    let id = history.conversation().unwrap().id;
    (inputs, history, id)
}

fn draft(text: &str, revision: u64) -> SavedDraft {
    SavedDraft {
        text: text.into(),
        status: DraftStatus {
            revision,
            ..DraftStatus::default()
        },
        ..SavedDraft::default()
    }
}

#[tokio::test]
async fn first_turn_is_not_required_for_a_truthful_recoverable_checkpoint() {
    let dir = tempfile::tempdir().unwrap();
    let (inputs, history, id) = history(&dir).await;
    assert_eq!(history.load_draft().await.unwrap().status.conversation, id);
    let mut message = draft("日本語\r\nNever sent", 1);
    message.attachments = pasted("clipboard.png", b"\x89PNG\r\n\x1a\nimage");
    let attachment_id = message.attachments[0].id().to_owned();
    history.save_draft(message.clone(), 0).await.unwrap();
    let conn = storage::open_readonly_db(&storage::sessions_db_path(&inputs).unwrap()).unwrap();
    let records = storage::query_sessions(&conn, &SessionFilter::new()).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].session_id, id);
    assert_eq!(records[0].ended_at, None);
    assert_eq!(records[0].outcome, None);
    assert_eq!(records[0].turns, Some(0));
    assert_eq!(records[0].model.as_deref(), Some("chosen-model"));
    assert!(!records[0].can_resume());
    assert!(records[0].updated_at >= records[0].started_at);
    drop(history); // No settlement: this models recovery from a lost process.
    let recovered = crate::application::saved_draft(&inputs, &id).unwrap();
    assert_eq!(recovered.status.conversation, id);
    assert_eq!(recovered.text, message.text);
    assert_eq!(recovered.attachments[0].id(), attachment_id);
    assert_eq!(
        recovered.attachments[0].content().bytes(),
        message.attachments[0].content().bytes()
    );
    assert!(crate::application::saved_draft(&inputs, "another-conversation").is_err());
}

#[tokio::test]
async fn independent_writers_compare_the_actual_durable_revision_and_keep_owner_identity() {
    let dir = tempfile::tempdir().unwrap();
    let (inputs, history, id) = history(&dir).await;
    history.save_draft(draft("original", 1), 0).await.unwrap();
    edit_saved(
        &inputs,
        &id,
        1,
        SavedEdit::Text("from another owner".into()),
    )
    .unwrap();
    assert!(
        history
            .save_draft(draft("late resident edit", 2), 1)
            .await
            .unwrap_err()
            .contains("changed")
    );
    assert_eq!(
        crate::application::saved_draft(&inputs, &id).unwrap().text,
        "from another owner"
    );
    let conn = storage::open_db(&storage::sessions_db_path(&inputs).unwrap()).unwrap();
    let mut record = storage::get_session(&conn, &id).unwrap().unwrap();
    record.workspace = "/another-project".into();
    record.updated_at = Utc::now();
    assert!(write(&conn, &record, &draft("foreign", 3), 2).is_err());
    assert_eq!(load(&conn, &id).unwrap().text, "from another owner");
}

#[tokio::test]
async fn offline_edit_remove_and_discard_preserve_stable_files_and_reject_stale_views() {
    let dir = tempfile::tempdir().unwrap();
    let (inputs, history, id) = history(&dir).await;
    let mut message = draft("before", 1);
    message.attachments = pasted("note.txt", b"original bytes");
    history.save_draft(message, 0).await.unwrap();
    let attachment_id = crate::application::saved_draft(&inputs, &id)
        .unwrap()
        .attachments[0]
        .id()
        .to_owned();
    let status = edit_saved(
        &inputs,
        &id,
        1,
        SavedEdit::Text("edited without a provider".into()),
    )
    .unwrap();
    let current = crate::application::saved_draft(&inputs, &id).unwrap();
    assert_eq!(status.conversation, id);
    assert_eq!(current.status.conversation, id);
    assert_eq!(current.attachments[0].id(), attachment_id);
    assert_eq!(current.attachments[0].content().bytes(), b"original bytes");
    assert!(edit_saved(&inputs, &id, 1, SavedEdit::Discard).is_err());
    let status = edit_saved(
        &inputs,
        &id,
        status.revision,
        SavedEdit::Remove(attachment_id),
    )
    .unwrap();
    assert!(
        crate::application::saved_draft(&inputs, &id)
            .unwrap()
            .attachments
            .is_empty()
    );
    edit_saved(&inputs, &id, status.revision, SavedEdit::Discard).unwrap();
    assert_eq!(
        crate::application::saved_draft(&inputs, &id).unwrap().text,
        ""
    );
}

#[tokio::test]
async fn corrupt_payload_is_reported_without_overwriting_the_record() {
    let dir = tempfile::tempdir().unwrap();
    let (inputs, history, id) = history(&dir).await;
    history.save_draft(draft("keep this", 1), 0).await.unwrap();
    let conn = storage::open_db(&storage::sessions_db_path(&inputs).unwrap()).unwrap();
    conn.execute(
        "UPDATE session_drafts SET attachments = ?1 WHERE session_id = ?2",
        params![b"invalid json".as_slice(), id],
    )
    .unwrap();
    assert!(crate::application::saved_draft(&inputs, &id).is_err());
    assert!(edit_saved(&inputs, &id, 1, SavedEdit::Text("replacement".into())).is_err());
    let text: String = conn
        .query_row(
            "SELECT text FROM session_drafts WHERE session_id=?1",
            [&id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(text, "keep this");
}

#[tokio::test]
async fn settlement_keeps_draft_and_records_a_real_end() {
    let dir = tempfile::tempdir().unwrap();
    let (inputs, history, id) = history(&dir).await;
    history
        .save_draft(draft("still unsent", 1), 0)
        .await
        .unwrap();
    history.settle(SessionOutcome::Stopped).await.unwrap();
    let conn = storage::open_readonly_db(&storage::sessions_db_path(&inputs).unwrap()).unwrap();
    let record = storage::get_session(&conn, &id).unwrap().unwrap();
    assert_eq!(record.outcome, Some(SessionOutcome::Stopped));
    assert_eq!(record.ended_at, Some(record.updated_at));
    assert_eq!(load(&conn, &id).unwrap().text, "still unsent");
}
