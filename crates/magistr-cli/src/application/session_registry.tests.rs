use super::*;
use magistr_application::sessions::{AgentEvent, PermissionChoice, PermissionKind};
use magistr_cockpit::cockpit::Msg;
use std::sync::Mutex;
use tokio::sync::Notify;

#[derive(Clone, Default)]
pub(crate) struct Log(pub(crate) Arc<Mutex<Vec<String>>>);
impl Log {
    pub(crate) fn note(&self, text: String) {
        self.0.lock().unwrap().push(text);
    }
    pub(crate) fn lines(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}
pub(crate) struct Hosted {
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), String>>>,
}
impl Drop for Hosted {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}
impl Settle for Hosted {
    fn settle(mut self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
        Box::pin(async move {
            let _ = self.stop.take().unwrap().send(());
            self.task
                .take()
                .unwrap()
                .await
                .map_err(|error| error.to_string())?
        })
    }
}
pub(crate) fn mutation_target(address: &SessionAddress) -> ConversationTarget {
    ConversationTarget {
        address: address.clone(),
        conversation: address.session.clone(),
    }
}

fn bind_opening(opening: &Opening) -> String {
    if let Some(conversation) = opening.observation.snapshot().conversation {
        return conversation.id;
    }
    let id = match &opening.choice {
        SessionOpening::Resume { id } => id.clone(),
        _ => opening.address.session.clone(),
    };
    opening
        .observation
        .bind_context_for_test(magistr_application::sessions::Conversation {
            id: id.clone(),
            purpose: magistr_application::sessions::SessionPurpose::Conversation,
            name: None,
            provider: "scripted".into(),
        });
    id
}

pub(crate) fn opener(log: Log) -> Open<Hosted> {
    Box::new(move |opening| {
        let log = log.clone();
        Box::pin(async move {
            let name = Path::new(&opening.address.workspace)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if name == "held" {
                return Err("provider unavailable".into());
            }
            let conversation = bind_opening(&opening);
            log.note(format!("open {name}"));
            let seed = ProjectSeed {
                name: name.clone(),
                dir: opening.address.workspace,
                ..ProjectSeed::default()
            };
            let (stop, mut stopped) = oneshot::channel();
            let mut commands = opening.commands;
            let inbox = opening.inbox.for_conversation(&conversation);
            let task = tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = &mut stopped => break,
                        command = commands.next() => {
                            let Some(command) = command else { break; };
                            let Some(command) = inbox.command_for(command, Some(&conversation)) else { continue; };
                            match command {
                            Cmd::Prompt(input) => {
                                let text = input.text;
                                log.note(format!("{name} got {text}"));
                                inbox.send(Msg::Agent(AgentEvent::Text(format!("{name}: {text}"))));
                                if text == "ask" {
                                    inbox.send(Msg::Agent(AgentEvent::PermissionRequest { detail: String::new(), id: 1, title: "Read".into(), choices: vec![PermissionChoice::new("allow", PermissionKind::AllowOnce)], edit: false, risk: magistr_domain::policy::RiskTier::Low, plan: None }));
                                }
                            }
                            Cmd::Permission { id, option_id } => {
                                log.note(format!("{name} answered {id} {option_id}"));
                                inbox.send(Msg::Agent(AgentEvent::PermissionCancelled { id }));
                            }
                            Cmd::Cancel => { log.note(format!("{name} cancelled")); }
                            Cmd::Loop(command) => { log.note(format!("{name} work {} {:?}", command.run_id.local(), command.action)); }
                            _ => {}
                            }
                        },
                    }
                }
                inbox.send(Msg::Agent(AgentEvent::Text(format!("{name} closing"))));
                log.note(format!("settled {name}"));
                if name == "broken-close" {
                    Err("fixture cleanup failed".into())
                } else {
                    Ok(())
                }
            });
            Ok((
                Hosted {
                    stop: Some(stop),
                    task: Some(task),
                },
                seed,
            ))
        })
    })
}
pub(crate) fn registry(log: Log) -> SessionRegistry {
    SessionRegistry::start_with(opener(log), Box::new(|| Box::pin(async { Ok(()) })))
}
pub(crate) fn projects() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    for name in ["first", "second", "held", "broken-close"] {
        std::fs::create_dir_all(tmp.path().join(name).join(".magistr")).unwrap();
        std::fs::write(
            tmp.path().join(name).join(".magistr/project.json"),
            serde_json::json!({
 "project": {"name": "fixture", "type": "cli"},
 "stack": {"language": "unknown", "framework": "none", "package_manager": "unknown", "test_framework": "none", "linter": "none", "formatter": "none"},
 "quality_gate": {"check": "", "test": "", "lint": ""},
 "format": {"command": "", "extensions": []},
 "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
 "reference_repos": [], "state_prefix": "fixture", "roadmap_exports": false
}).to_string(),
        )
        .unwrap();
    }
    std::fs::create_dir(tmp.path().join("bare")).unwrap();
    tmp
}
async fn until(
    reader: &mut SessionSubscription,
    predicate: impl Fn(&SessionSnapshot) -> bool,
) -> SessionSnapshot {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let snapshot = reader.snapshot();
            if predicate(&snapshot) {
                return snapshot;
            }
            reader.changed().await.expect("owner retains reader");
        }
    })
    .await
    .unwrap()
}

#[test]
fn stop_invalidates_delayed_preparation_and_concurrent_sends_cannot_overtake() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let tmp = projects();
        let log = Log::default();
        let registry = registry(log.clone());
        let client = registry.client();
        let address = client.open(tmp.path().join("first"), SessionOpening::Launch).await.unwrap();
        let mut reader = client.subscribe(&address).await.unwrap();
        // Hold the sole source worker after admission so cancellation occurs
        // deterministically before exact-byte preparation can complete.
        let (started, ready) = oneshot::channel();
        let (release, held) = std::sync::mpsc::channel();
        let worker = tokio::task::spawn_blocking(move || {
            let _ = started.send(());
            let _ = held.recv();
        });
        ready.await.unwrap();
        let sender = client.clone();
        let target = address.clone();
        let pending = tokio::spawn(async move {
            sender.send(&mutation_target(&target), "stopped message".into(), Vec::new()).await
        });
        until(&mut reader, |snapshot| snapshot.seed.draft.as_deref() == Some("stopped message")).await;
        assert!(client.send(&mutation_target(&address), "later message".into(), Vec::new())
            .await.unwrap_err().contains("still being checked"));
        assert!(client.command(&mutation_target(&address), Cmd::Prompt("cannot overtake".into()))
            .await.unwrap_err().contains("still being checked"));
        client.command(&mutation_target(&address), Cmd::Cancel).await.unwrap();
        release.send(()).unwrap();
        worker.await.unwrap();
        assert!(pending.await.unwrap().unwrap_err().contains("was stopped"));
        assert_eq!(reader.snapshot().seed.draft.as_deref(), Some("stopped message"));
        client.send(&mutation_target(&address), "send after stopping".into(), Vec::new()).await.unwrap();
        until(&mut reader, |snapshot| snapshot.messages.iter().any(|event|
            matches!(&event.message, Msg::Agent(AgentEvent::Text(text)) if text == "first: send after stopping")
        )).await;
        assert!(!log.lines().iter().any(|line| line == "first got stopped message"));
        assert!(log.lines().iter().any(|line| line == "first cancelled"));
        registry.shutdown().await.unwrap();
    });
}
#[tokio::test(flavor = "multi_thread")]
async fn addressed_sessions_in_two_workspaces_survive_disconnect_and_route_permissions() {
    let tmp = projects();
    let log = Log::default();
    let registry = registry(log.clone());
    let client = registry.client();
    let a = client
        .open(
            tmp.path().join("first"),
            crate::application::SessionOpening::Launch,
        )
        .await
        .unwrap();
    let b = client
        .open(
            tmp.path().join("second"),
            crate::application::SessionOpening::Launch,
        )
        .await
        .unwrap();
    let c = client
        .open(
            tmp.path().join("first"),
            crate::application::SessionOpening::Launch,
        )
        .await
        .unwrap();
    assert_ne!(a, c);
    assert_eq!(a.workspace, c.workspace);
    let mut reader = client.subscribe(&a).await.unwrap();
    client
        .command(&mutation_target(&a), Cmd::Prompt("ask".into()))
        .await
        .unwrap();
    until(&mut reader, |snapshot| snapshot.pending.contains_key(&1)).await;
    drop(reader);
    client
        .draft(&mutation_target(&a), 0, "unsent first".into())
        .await
        .unwrap();
    client
        .command(&mutation_target(&b), Cmd::Prompt("other".into()))
        .await
        .unwrap();
    assert!(
        client
            .command(
                &mutation_target(&b),
                Cmd::Permission {
                    id: 1,
                    option_id: "allow".into()
                }
            )
            .await
            .unwrap_err()
            .contains("no longer")
    );
    assert!(
        client
            .command(
                &mutation_target(&a),
                Cmd::Permission {
                    id: 1,
                    option_id: "unoffered".into()
                }
            )
            .await
            .is_err()
    );
    client
        .command(
            &mutation_target(&a),
            Cmd::Permission {
                id: 1,
                option_id: "allow".into(),
            },
        )
        .await
        .unwrap();
    let mut reader = client.subscribe(&a).await.unwrap();
    let snapshot = until(&mut reader, |snapshot| !snapshot.pending.contains_key(&1)).await;
    assert_eq!(snapshot.seed.draft.as_deref(), Some("unsent first"));
    client
        .command(&mutation_target(&c), Cmd::Cancel)
        .await
        .unwrap();
    assert!(
        client
            .open(
                tmp.path().join("held"),
                crate::application::SessionOpening::Launch
            )
            .await
            .unwrap_err()
            .contains("provider unavailable")
    );
    assert_eq!(client.list().await.unwrap().len(), 3);
    client.close(&a).await.unwrap();
    assert!(
        client
            .command(&mutation_target(&a), Cmd::Cancel)
            .await
            .unwrap_err()
            .contains("no longer")
    );
    let mut forged = c.clone();
    forged.workspace = b.workspace.clone();
    assert!(
        client
            .command(&mutation_target(&forged), Cmd::Cancel)
            .await
            .is_err()
    );
    client
        .command(&mutation_target(&b), Cmd::Prompt("still alive".into()))
        .await
        .unwrap();
    registry.shutdown().await.unwrap();
    assert_eq!(
        log.lines()
            .iter()
            .filter(|line| line.starts_with("settled"))
            .count(),
        3
    );
}

#[tokio::test]
async fn cleanup_failure_is_reported_after_every_other_host_is_settled() {
    let tmp = projects();
    let log = Log::default();
    let registry = registry(log.clone());
    let client = registry.client();
    client
        .open(
            tmp.path().join("first"),
            crate::application::SessionOpening::Launch,
        )
        .await
        .unwrap();
    client
        .open(
            tmp.path().join("broken-close"),
            crate::application::SessionOpening::Launch,
        )
        .await
        .unwrap();
    client
        .open(
            tmp.path().join("second"),
            crate::application::SessionOpening::Launch,
        )
        .await
        .unwrap();
    assert!(
        registry
            .shutdown()
            .await
            .unwrap_err()
            .contains("fixture cleanup failed")
    );
    assert_eq!(
        log.lines()
            .iter()
            .filter(|line| line.starts_with("settled"))
            .count(),
        3
    );
}

#[tokio::test]
async fn opening_a_provider_does_not_block_controls_for_an_existing_session() {
    let tmp = projects();
    let log = Log::default();
    let mut normal = opener(log.clone());
    let release = Arc::new(Notify::new());
    let gate = release.clone();
    let mut count = 0;
    let open: Open<Hosted> = Box::new(move |opening| {
        count += 1;
        let opened = normal(opening);
        let gate = gate.clone();
        let waiting = count == 2;
        Box::pin(async move {
            if waiting {
                gate.notified().await;
            }
            opened.await
        })
    });
    let registry = SessionRegistry::start_with(open, Box::new(|| Box::pin(async { Ok(()) })));
    let client = registry.client();
    let a = client
        .open(
            tmp.path().join("first"),
            crate::application::SessionOpening::Launch,
        )
        .await
        .unwrap();
    let opening = {
        let client = client.clone();
        let root = tmp.path().join("second");
        tokio::spawn(async move {
            client
                .open(root, crate::application::SessionOpening::Launch)
                .await
        })
    };
    tokio::task::yield_now().await;
    tokio::time::timeout(
        Duration::from_secs(1),
        client.command(&mutation_target(&a), Cmd::Cancel),
    )
    .await
    .unwrap()
    .unwrap();
    release.notify_one();
    opening.await.unwrap().unwrap();
    registry.shutdown().await.unwrap();
}

#[tokio::test]
async fn a_full_application_queue_returns_an_explicit_retry_without_losing_input() {
    let (sender, _receiver) = mpsc::channel(1);
    let client = SessionClient {
        sender,
        catalog: watch::channel(super::super::SessionCatalog::default()).1,
    };
    let (reply, _) = oneshot::channel();
    client.post(Request::List { reply }).unwrap();
    let address = SessionAddress {
        project: "p".into(),
        workspace: "w".into(),
        session: "s".into(),
    };
    assert!(
        client
            .command(
                &mutation_target(&address),
                Cmd::Prompt("keep this draft".into())
            )
            .await
            .unwrap_err()
            .contains("busy")
    );
}

#[tokio::test]
async fn oversized_input_is_refused_before_entering_the_owner_queue() {
    let (sender, mut receiver) = mpsc::channel(1);
    let client = SessionClient {
        sender,
        catalog: watch::channel(super::super::SessionCatalog::default()).1,
    };
    let address = SessionAddress {
        project: "p".into(),
        workspace: "w".into(),
        session: "s".into(),
    };
    let text = "é".repeat(524_289);
    assert!(
        tokio::time::timeout(
            Duration::from_millis(100),
            client.draft(&mutation_target(&address), 0, text.clone())
        )
        .await
        .expect("must refuse before posting")
        .unwrap_err()
        .contains("too long")
    );
    assert!(
        tokio::time::timeout(
            Duration::from_millis(100),
            client.command(&mutation_target(&address), Cmd::Prompt(text.into()))
        )
        .await
        .expect("must refuse before posting")
        .unwrap_err()
        .contains("too long")
    );
    assert!(
        receiver.try_recv().is_err(),
        "oversized data must not be retained"
    );
}

#[tokio::test]
async fn catalog_tracks_background_attention_without_token_or_draft_notifications() {
    let tmp = projects();
    let registry = registry(Log::default());
    let client = registry.client();
    let a = client
        .open(
            tmp.path().join("first"),
            crate::application::SessionOpening::Launch,
        )
        .await
        .unwrap();
    let b = client
        .open(
            tmp.path().join("second"),
            crate::application::SessionOpening::Launch,
        )
        .await
        .unwrap();
    let mut catalog = client.catalog();
    assert_eq!(catalog.snapshot().sessions.len(), 2);
    client
        .command(&mutation_target(&b), Cmd::Prompt("ask".into()))
        .await
        .unwrap();
    let update = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let next = catalog.changed().await.unwrap();
            if next
                .sessions
                .iter()
                .any(|session| session.address == b && session.requests.len() == 1)
            {
                break next;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(
        update
            .sessions
            .iter()
            .find(|session| session.address == a)
            .unwrap()
            .status,
        super::super::SessionStatus::Ready
    );
    client
        .draft(&mutation_target(&b), 0, "background draft".into())
        .await
        .unwrap();
    for _ in 0..100 {
        client
            .command(&mutation_target(&b), Cmd::Prompt("token".into()))
            .await
            .unwrap();
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(50), catalog.changed())
            .await
            .is_err(),
        "streaming text and drafts must not wake the catalog"
    );
    client
        .command(
            &mutation_target(&b),
            Cmd::Permission {
                id: 1,
                option_id: "allow".into(),
            },
        )
        .await
        .unwrap();
    let next = tokio::time::timeout(Duration::from_secs(3), catalog.changed())
        .await
        .unwrap()
        .unwrap();
    assert!(
        next.sessions
            .iter()
            .find(|session| session.address == b)
            .unwrap()
            .requests
            .is_empty()
    );
    assert!(
        client
            .command(
                &mutation_target(&b),
                Cmd::Permission {
                    id: 1,
                    option_id: "allow".into()
                }
            )
            .await
            .is_err()
    );
    client.close(&b).await.unwrap();
    assert_eq!(client.list().await.unwrap().len(), 1);
    registry.shutdown().await.unwrap();
}

#[tokio::test]
async fn refused_duplicate_permission_does_not_release_an_accepted_reservation() {
    let tmp = projects();
    let (sent, mut opened) = mpsc::unbounded_channel();
    struct Held(Opening);
    impl Settle for Held {
        fn settle(self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
            Box::pin(async move {
                drop(self.0);
                Ok(())
            })
        }
    }
    let registry = SessionRegistry::start_with(
        Box::new(move |opening| {
            bind_opening(&opening);
            sent.send(Arc::clone(&opening.observation)).unwrap();
            Box::pin(async { Ok((Held(opening), ProjectSeed::default())) })
        }),
        Box::new(|| Box::pin(async { Ok(()) })),
    );
    let client = registry.client();
    let address = client
        .open(
            tmp.path().join("first"),
            crate::application::SessionOpening::Launch,
        )
        .await
        .unwrap();
    let observation = opened.recv().await.unwrap();
    use magistr_cockpit_runtime::channel::ObservationSink;
    observation.send(Msg::Agent(AgentEvent::PermissionRequest {
        id: 1,
        title: "Read".into(),
        detail: String::new(),
        choices: vec![PermissionChoice::new("allow", PermissionKind::AllowOnce)],
        edit: false,
        risk: magistr_domain::policy::RiskTier::Low,
        plan: None,
    }));
    let answer = || Cmd::Permission {
        id: 1,
        option_id: "allow".into(),
    };
    client
        .command(&mutation_target(&address), answer())
        .await
        .unwrap();
    assert!(
        client
            .command(&mutation_target(&address), answer())
            .await
            .is_err()
    );
    assert!(
        client
            .command(&mutation_target(&address), answer())
            .await
            .is_err(),
        "a rejected retry cannot reopen an accepted answer"
    );
    assert_eq!(observation.snapshot().pending_requests().count(), 0);
    registry.shutdown().await.unwrap();
}

#[tokio::test]
async fn catalog_keeps_closing_visible_until_the_addressed_owner_settles() {
    let tmp = projects();
    struct Closing {
        _opening: Opening,
        release: Arc<Notify>,
    }
    impl Settle for Closing {
        fn settle(self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
            Box::pin(async move {
                self.release.notified().await;
                Ok(())
            })
        }
    }
    let release = Arc::new(Notify::new());
    let gate = Arc::clone(&release);
    let registry = SessionRegistry::start_with(
        Box::new(move |opening| {
            let release = Arc::clone(&gate);
            Box::pin(async move {
                opening.observation.bind_context_for_test(
                    magistr_application::sessions::Conversation {
                        id: "fresh-saved-conversation".into(),
                        purpose: magistr_application::sessions::SessionPurpose::Conversation,
                        name: None,
                        provider: "copilot".into(),
                    },
                );
                Ok((
                    Closing {
                        _opening: opening,
                        release,
                    },
                    ProjectSeed::default(),
                ))
            })
        }),
        Box::new(|| Box::pin(async { Ok(()) })),
    );
    let client = registry.client();
    let address = client
        .open(
            tmp.path().join("first"),
            crate::application::SessionOpening::Launch,
        )
        .await
        .unwrap();
    let target = client
        .subscribe(&address)
        .await
        .unwrap()
        .snapshot()
        .target()
        .unwrap();
    let mut catalog = client.catalog();
    catalog.snapshot();
    let addressed = address.clone();
    let other = client.clone();
    let closing = tokio::spawn(async move { other.close(&addressed).await });
    let snapshot = tokio::time::timeout(Duration::from_secs(3), catalog.changed())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.sessions[0].address, address);
    assert_eq!(snapshot.sessions[0].label(), "Closing");
    assert!(!closing.is_finished());
    assert!(client.command(&target, Cmd::Cancel).await.is_err());
    let duplicate = client
        .open(
            tmp.path().join("first"),
            SessionOpening::Resume {
                id: "fresh-saved-conversation".into(),
            },
        )
        .await;
    assert!(matches!(duplicate, Err(error) if error.contains("already open")));
    release.notify_one();
    closing.await.unwrap().unwrap();
    let snapshot = tokio::time::timeout(Duration::from_secs(3), catalog.changed())
        .await
        .unwrap()
        .unwrap();
    assert!(snapshot.sessions.is_empty());
    registry.shutdown().await.unwrap();
}
#[tokio::test]
async fn saved_conversation_claims_are_unique_and_release_after_close_or_refusal() {
    let tmp = projects();
    let registry = registry(Log::default());
    let client = registry.client();
    let resume = || SessionOpening::Resume { id: "saved".into() };
    let address = client
        .open(tmp.path().join("first"), resume())
        .await
        .unwrap();
    assert!(
        client
            .open(tmp.path().join("first"), resume())
            .await
            .unwrap_err()
            .contains("already open")
    );
    client.close(&address).await.unwrap();
    let reopened = client
        .open(tmp.path().join("first"), resume())
        .await
        .unwrap();
    assert_ne!(address, reopened);
    client.close(&reopened).await.unwrap();
    for _ in 0..2 {
        assert_eq!(
            client
                .open(tmp.path().join("held"), resume())
                .await
                .unwrap_err(),
            "provider unavailable"
        );
    }
    registry.shutdown().await.unwrap();
}

#[path = "session_registry_work.tests.rs"]
mod work_tests;
