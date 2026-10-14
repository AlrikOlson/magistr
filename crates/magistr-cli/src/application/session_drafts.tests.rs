use super::*;
use crate::agent::persistence::{SessionHistory, storage};
use crate::application::{
    session_attachments::{AttachmentSelection, prepare_send},
    session_observation::SessionObservation,
};
use magistr_application::sessions::{AgentEvent, PromptInput};
use magistr_cockpit::cockpit::Msg;
use magistr_cockpit_runtime::channel::ObservationSink;
use magistr_domain::identity::SessionAddress;
use magistr_workspace::service::{WorkspaceOpenOptions, WorkspaceService};

fn inputs(dir: &tempfile::TempDir) -> ProjectInputs {
    ProjectInputs::from_lookup(dir.path(), |key| {
        (key == "XDG_DATA_HOME").then(|| dir.path().join("state").display().to_string())
    })
}

async fn owner(
    dir: &tempfile::TempDir,
) -> (
    ProjectInputs,
    SessionHistory,
    Arc<SessionObservation>,
    String,
) {
    let inputs = inputs(dir);
    let history = SessionHistory::new();
    history
        .begin(&inputs, dir.path(), "scripted", None)
        .await
        .unwrap();
    let id = history.conversation().unwrap().id;
    let owner = Arc::new(SessionObservation::new(SessionAddress {
        project: "project".into(),
        workspace: dir.path().display().to_string(),
        session: "resident".into(),
    }));
    owner.bind_history(history.clone()).await.unwrap();
    owner.activate_context(&id).unwrap();
    assert!(owner.flush_draft_for(&id).await.error.is_none());
    (inputs, history, owner, id)
}

#[test]
fn reading_a_saved_draft_does_not_create_missing_storage() {
    let dir = tempfile::tempdir().unwrap();
    let inputs = inputs(&dir);
    assert!(saved_draft(&inputs, "missing").is_err());
    assert!(!storage::sessions_db_path(&inputs).unwrap().exists());
}

#[tokio::test]
async fn coalesced_edits_flush_the_latest_revision_and_restart_with_the_same_conversation() {
    let dir = tempfile::tempdir().unwrap();
    let (inputs, _, owner, id) = owner(&dir).await;
    for revision in 0..200 {
        owner
            .edit_draft_for(&id, revision, format!("message {}", revision + 1), false)
            .unwrap();
    }
    let status = owner.flush_draft_for(&id).await;
    assert_eq!(status.revision, 200);
    assert_eq!(status.saved_revision, 200);
    assert!(status.error.is_none());
    let recovered = saved_draft(&inputs, &id).unwrap();
    assert_eq!(recovered.text, "message 200");
    assert_eq!(recovered.status.revision, 200);
    assert_eq!(recovered.status.save_attempt, 0);
    let conn = storage::open_readonly_db(&storage::sessions_db_path(&inputs).unwrap()).unwrap();
    let history = SessionHistory::from_record(storage::get_session(&conn, &id).unwrap().unwrap());
    history
        .begin(&inputs, dir.path(), "scripted", None)
        .await
        .unwrap();
    let restored = Arc::new(SessionObservation::new(SessionAddress {
        project: "project".into(),
        workspace: dir.path().display().to_string(),
        session: "new-resident".into(),
    }));
    restored.bind_history(history).await.unwrap();
    restored.activate_context(&id).unwrap();
    restored.seed(magistr_cockpit::cockpit::ProjectSeed::default());
    assert_eq!(
        restored.snapshot().seed.draft.as_deref(),
        Some("message 200")
    );
    assert_eq!(restored.snapshot().draft_revision, 200);
    assert!(restored.flush_draft_for(&id).await.error.is_none());
}

#[tokio::test]
async fn refused_storage_is_visible_and_retrying_identical_text_really_saves_it() {
    let dir = tempfile::tempdir().unwrap();
    let (inputs, _, owner, id) = owner(&dir).await;
    let initial = owner.snapshot().draft_status();
    let lock = storage::open_db(&storage::sessions_db_path(&inputs).unwrap()).unwrap();
    lock.execute_batch("BEGIN IMMEDIATE;").unwrap();
    owner
        .edit_draft_for(&id, 0, "keep in the editor".into(), false)
        .unwrap();
    let failed = owner.flush_draft_for(&id).await;
    assert_eq!(failed.revision, 1);
    assert_eq!(failed.saved_revision, 0);
    assert!(failed.save_attempt > initial.save_attempt);
    assert_eq!(owner.snapshot().draft_status(), failed);
    let message = failed.error.as_deref().unwrap();
    assert!(message.contains("could not be saved"));
    assert!(message.contains("Try again"));
    assert!(!message.contains("database"));
    assert_eq!(
        owner.snapshot().seed.draft.as_deref(),
        Some("keep in the editor")
    );
    lock.execute_batch("ROLLBACK;").unwrap();
    owner
        .edit_draft_for(&id, 1, "keep in the editor".into(), false)
        .unwrap();
    let retried = owner.flush_draft_for(&id).await;
    assert_eq!(retried.revision, failed.revision);
    assert_eq!(retried.saved_revision, 1);
    assert!(retried.save_attempt > failed.save_attempt);
    assert!(retried.error.is_none());
    assert_eq!(owner.snapshot().draft_status(), retried);
    owner
        .edit_draft_for(&id, 1, "keep in the editor".into(), false)
        .unwrap();
    let repeated = owner.flush_draft_for(&id).await;
    assert_eq!(repeated.revision, retried.revision);
    assert_eq!(repeated.saved_revision, retried.saved_revision);
    assert!(repeated.save_attempt > retried.save_attempt);
    assert_eq!(owner.snapshot().draft_status(), repeated);
    assert_eq!(
        serde_json::to_value(&repeated).unwrap()["save_attempt"],
        repeated.save_attempt,
    );
    assert_eq!(
        saved_draft(&inputs, &id).unwrap().text,
        "keep in the editor"
    );
}

#[tokio::test]
async fn accepted_input_is_durably_cleared_and_late_editor_saves_cannot_resurrect_it() {
    let dir = tempfile::tempdir().unwrap();
    let (inputs, _, owner, id) = owner(&dir).await;
    owner
        .edit_draft_for(&id, 0, "send me".into(), false)
        .unwrap();
    owner.flush_draft_for(&id).await;
    let input: PromptInput = "send me".into();
    owner.send(Msg::InputAccepted(input.receipt()));
    assert!(
        owner
            .edit_draft_for(&id, 1, "send me".into(), false)
            .is_err()
    );
    assert_eq!(owner.flush_draft_for(&id).await.saved_revision, 2);
    assert_eq!(saved_draft(&inputs, &id).unwrap().text, "");
    owner.send(Msg::Agent(AgentEvent::PromptRejected {
        input: input.clone(),
        error: "queue stopped".into(),
    }));
    assert_eq!(owner.flush_draft_for(&id).await.saved_revision, 3);
    assert_eq!(saved_draft(&inputs, &id).unwrap().text, "send me");
    owner
        .edit_draft_for(&id, 3, "a newer thought".into(), false)
        .unwrap();
    owner.send(Msg::InputAccepted(input.receipt()));
    owner.flush_draft_for(&id).await;
    assert_eq!(saved_draft(&inputs, &id).unwrap().text, "a newer thought");
    owner.edit_draft_for(&id, 4, String::new(), true).unwrap();
    assert_eq!(owner.flush_draft_for(&id).await.saved_revision, 5);
    assert_eq!(saved_draft(&inputs, &id).unwrap().text, "");
}

#[tokio::test]
async fn restored_files_keep_exact_bytes_but_a_changed_or_missing_source_still_refuses_send() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notes.txt");
    std::fs::write(&path, "日本語\r\n").unwrap();
    let service = WorkspaceService::open(
        WorkspaceOpenOptions::new(dir.path(), "/nonexistent/draft-worker")
            .with_network_override("0"),
    )
    .await
    .unwrap();
    let (inputs, _, owner, id) = owner(&dir).await;
    let attachments = AttachmentSelection::Files(vec![path.clone()])
        .prepare(Some(&service))
        .unwrap();
    let selection = attachments[0].id().to_owned();
    owner.add_attachments_for(&id, attachments).unwrap();
    owner.flush_draft_for(&id).await;
    let restored = saved_draft(&inputs, &id).unwrap();
    assert_eq!(restored.attachments[0].id(), selection);
    assert_eq!(
        restored.attachments[0].content().bytes(),
        "日本語\r\n".as_bytes()
    );
    assert!(prepare_send(restored.text.clone(), &restored.attachments, Some(&service)).is_ok());
    std::fs::write(&path, "changed").unwrap();
    assert!(
        prepare_send(restored.text.clone(), &restored.attachments, Some(&service))
            .unwrap_err()
            .contains("changed")
    );
    std::fs::remove_file(&path).unwrap();
    assert!(
        prepare_send(restored.text.clone(), &restored.attachments, Some(&service))
            .unwrap_err()
            .contains("missing")
    );
    assert_eq!(
        saved_draft(&inputs, &id).unwrap().attachments[0]
            .content()
            .bytes(),
        "日本語\r\n".as_bytes()
    );
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn registry_reply_keeps_its_accepted_revision_and_saved_edits_share_the_owner() {
    use crate::application::{
        SessionOpening, SessionRegistry,
        session_registry::tests::{Log, opener, projects},
    };
    let dir = projects();
    let root = dir.path().join("first");
    let inputs = inputs(&dir);
    let history = SessionHistory::new();
    history
        .begin(&inputs, &root, "scripted", None)
        .await
        .unwrap();
    let id = history.conversation().unwrap().id;
    let observation = Arc::new(std::sync::Mutex::new(None::<Arc<SessionObservation>>));
    let selected = Arc::clone(&observation);
    let mut open = opener(Log::default());
    let registry = SessionRegistry::start_with(
        Box::new(move |opening| {
            let owner = Arc::clone(&opening.observation);
            *selected.lock().unwrap() = Some(Arc::clone(&owner));
            let history = history.clone();
            let opened = open(opening);
            Box::pin(async move {
                owner.bind_history(history.clone()).await?;
                owner.activate_context(&history.id())?;
                owner.flush_draft_for(&history.id()).await;
                owner.send(Msg::Agent(AgentEvent::Conversation(
                    history.conversation().unwrap(),
                )));
                opened.await
            })
        }),
        Box::new(|| Box::pin(async { Ok(()) })),
    );
    let client = registry.client();
    let address = client.open(&root, SessionOpening::Launch).await.unwrap();
    let owner = observation.lock().unwrap().clone().unwrap();
    let target = owner.snapshot().target().unwrap();
    assert!(
        client
            .save_saved_draft(inputs.clone(), id.clone(), 0, "wrong path".into())
            .await
            .unwrap_err()
            .contains("open")
    );
    let lock = storage::open_db(&storage::sessions_db_path(&inputs).unwrap()).unwrap();
    lock.execute_batch("BEGIN IMMEDIATE;").unwrap();
    let saving = {
        let client = client.clone();
        let target = target.clone();
        tokio::spawn(async move { client.draft(&target, 0, "send this".into()).await })
    };
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while owner.snapshot().draft_revision != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    owner.send(Msg::InputAccepted(PromptInput::from("send this").receipt()));
    lock.execute_batch("ROLLBACK;").unwrap();
    let status = saving.await.unwrap().unwrap();
    assert_eq!(
        status.revision, 1,
        "the reply only acknowledges its own edit"
    );
    let completed = owner.flush_draft_for(&id).await;
    assert_eq!(completed.saved_revision, 2);
    assert!(status.save_attempt > 0);
    assert!(status.save_attempt <= completed.save_attempt);
    assert_eq!(owner.snapshot().draft_status(), completed);
    assert_eq!(saved_draft(&inputs, &id).unwrap().text, "");
    assert!(client.draft(&target, 1, "late save".into()).await.is_err());
    client.close(&address).await.unwrap();
    let status = client
        .save_saved_draft(
            inputs.clone(),
            id.clone(),
            2,
            "editable without an agent".into(),
        )
        .await
        .unwrap();
    assert_eq!(status.saved_revision, 3);
    assert_eq!(status.save_attempt, 0);
    assert_eq!(
        saved_draft(&inputs, &id).unwrap().text,
        "editable without an agent"
    );
    client
        .discard_saved_draft(inputs.clone(), id.clone(), 3)
        .await
        .unwrap();
    registry.shutdown().await.unwrap();
    assert_eq!(saved_draft(&inputs, &id).unwrap().text, "");
}
