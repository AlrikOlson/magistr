use super::*;
use magistr_application::sessions::{
    AgentEvent, Conversation, PermissionChoice, PermissionKind, SessionPurpose,
};
use magistr_cockpit::cockpit::Msg;

struct Held;
impl Settle for Held {
    fn settle(self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        Box::pin(async { Ok(()) })
    }
}
fn bind(observation: &SessionObservation, id: &str) {
    observation.bind_context_for_test(Conversation {
        id: id.into(),
        purpose: SessionPurpose::Conversation,
        name: None,
        provider: "scripted".into(),
    });
}
fn registry() -> (SessionRegistry, mpsc::UnboundedReceiver<Opening>) {
    let (captured, opened) = mpsc::unbounded_channel();
    let registry = SessionRegistry::start_with(
        Box::new(move |opening| {
            bind(&opening.observation, "a");
            assert!(captured.send(opening).is_ok());
            Box::pin(async { Ok((Held, ProjectSeed::default())) })
        }),
        Box::new(|| Box::pin(async { Ok(()) })),
    );
    (registry, opened)
}
fn permission(inbox: &Inbox, id: &str) {
    inbox
        .for_conversation(id)
        .send(Msg::Agent(AgentEvent::PermissionRequest {
            id: 7,
            title: "Read".into(),
            detail: String::new(),
            choices: vec![PermissionChoice::new("allow", PermissionKind::AllowOnce)],
            edit: false,
            risk: magistr_domain::policy::RiskTier::Low,
            plan: None,
        }));
}
async fn draft_is(observation: &SessionObservation, id: &str, text: &str) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while observation
            .context_snapshot(id)
            .unwrap()
            .seed
            .draft
            .as_deref()
            != Some(text)
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn parked_draft_edits_keep_their_owner_and_hold_the_saved_conversation_claim() {
    let tmp = tests::projects();
    let (registry, mut captured) = registry();
    let client = registry.client();
    let root = tmp.path().join("first");
    let address = client.open(&root, SessionOpening::Launch).await.unwrap();
    let opening = captured.recv().await.unwrap();
    let a = opening.observation.snapshot().target().unwrap();
    bind(&opening.observation, "b");
    let b = opening.observation.snapshot().target().unwrap();
    assert_eq!(
        opening.observation.summary().owned_conversations,
        ["a", "b"]
    );
    client
        .draft(&b, 0, "B stays selected".into())
        .await
        .unwrap();
    let before = opening.observation.snapshot();
    let saved = client.draft(&a, 0, "late A edit".into()).await.unwrap();
    assert_eq!(saved.conversation, "a");
    assert_eq!(saved.revision, 1);
    assert_eq!(opening.observation.snapshot(), before);
    assert_eq!(
        opening
            .observation
            .context_snapshot("a")
            .unwrap()
            .seed
            .draft
            .as_deref(),
        Some("late A edit")
    );
    assert!(
        client
            .open(&root, SessionOpening::Resume { id: "a".into() })
            .await
            .unwrap_err()
            .contains("already open")
    );
    let inputs = crate::config::inputs::ProjectInputs::from_lookup(&root, |_| None);
    assert!(
        client
            .save_saved_draft(inputs, "a".into(), 1, "second writer".into())
            .await
            .unwrap_err()
            .contains("open")
    );
    assert!(
        client
            .command(&a, Cmd::Prompt("wrong conversation".into()))
            .await
            .is_err()
    );
    assert_eq!(client.list().await.unwrap()[0].address, address);
    opening.observation.activate_context("a").unwrap();
    assert_eq!(
        opening.observation.snapshot().seed.draft.as_deref(),
        Some("late A edit")
    );
    registry.shutdown().await.unwrap();
}

#[test]
fn delayed_preparation_cannot_reach_the_new_conversation_or_block_its_send() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let tmp = tests::projects();
        let (registry, mut captured) = registry();
        let client = registry.client();
        client
            .open(tmp.path().join("first"), SessionOpening::Launch)
            .await
            .unwrap();
        let mut opening = captured.recv().await.unwrap();
        let a = opening.observation.snapshot().target().unwrap();
        let (started, ready) = oneshot::channel();
        let (release, held) = std::sync::mpsc::channel();
        let worker = tokio::task::spawn_blocking(move || {
            started.send(()).unwrap();
            held.recv().unwrap();
        });
        ready.await.unwrap();
        let first = {
            let client = client.clone();
            tokio::spawn(async move { client.send(&a, "A waiting".into(), Vec::new()).await })
        };
        draft_is(&opening.observation, "a", "A waiting").await;
        bind(&opening.observation, "b");
        let b = opening.observation.snapshot().target().unwrap();
        let second = {
            let client = client.clone();
            tokio::spawn(async move { client.send(&b, "B ready".into(), Vec::new()).await })
        };
        draft_is(&opening.observation, "b", "B ready").await;
        assert!(!first.is_finished());
        assert!(!second.is_finished());
        release.send(()).unwrap();
        worker.await.unwrap();
        assert!(first.await.unwrap().unwrap_err().contains("was stopped"));
        second.await.unwrap().unwrap();
        let queued = tokio::time::timeout(Duration::from_secs(3), opening.commands.next())
            .await
            .unwrap()
            .unwrap();
        let dispatched = opening.inbox.command_for(queued, Some("b"));
        assert!(matches!(dispatched, Some(Cmd::Prompt(input)) if input.text == "B ready"));
        assert!(
            tokio::time::timeout(Duration::from_millis(20), opening.commands.next())
                .await
                .is_err()
        );
        assert_eq!(
            opening
                .observation
                .context_snapshot("a")
                .unwrap()
                .seed
                .draft
                .as_deref(),
            Some("A waiting")
        );
        registry.shutdown().await.unwrap();
    });
}

#[tokio::test]
async fn queued_permission_rejection_releases_only_its_original_conversation() {
    let tmp = tests::projects();
    let (registry, mut captured) = registry();
    let client = registry.client();
    client
        .open(tmp.path().join("first"), SessionOpening::Launch)
        .await
        .unwrap();
    let mut opening = captured.recv().await.unwrap();
    let a = opening.observation.snapshot().target().unwrap();
    permission(&opening.inbox, "a");
    client
        .command(
            &a,
            Cmd::Permission {
                id: 7,
                option_id: "allow".into(),
            },
        )
        .await
        .unwrap();
    assert!(opening.observation.is_answered("a", 7));
    bind(&opening.observation, "b");
    permission(&opening.inbox, "b");
    let before = opening.observation.snapshot();
    let queued = opening.commands.next().await.unwrap();
    assert!(opening.inbox.command_for(queued, Some("b")).is_none());
    assert!(!opening.observation.is_answered("a", 7));
    assert_eq!(
        opening
            .observation
            .context_snapshot("a")
            .unwrap()
            .pending_requests()
            .count(),
        1
    );
    assert_eq!(opening.observation.snapshot(), before);
    assert_eq!(before.pending_requests().count(), 1);
    opening.observation.activate_context("a").unwrap();
    client
        .command(
            &a,
            Cmd::Permission {
                id: 7,
                option_id: "allow".into(),
            },
        )
        .await
        .unwrap();
    let queued = opening.commands.next().await.unwrap();
    opening.observation.activate_context("b").unwrap();
    let before = opening.observation.snapshot();
    // A bounded relay may return the still-wrapped command before dispatch.
    opening.inbox.send(Msg::CommandFailed {
        cmd: queued,
        error: "The session is busy. Try the action again.".into(),
    });
    assert!(!opening.observation.is_answered("a", 7));
    assert_eq!(opening.observation.snapshot(), before);
    assert_eq!(before.pending_requests().count(), 1);
    registry.shutdown().await.unwrap();
}
