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
            log.note(format!("open {name}"));
            let seed = ProjectSeed {
                name: name.clone(),
                dir: opening.address.workspace,
                ..ProjectSeed::default()
            };
            let (stop, mut stopped) = oneshot::channel();
            let mut commands = opening.commands;
            let inbox = opening.inbox;
            let task = tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = &mut stopped => break,
                        command = commands.next() => match command {
                            Some(Cmd::Prompt(text)) => {
                                log.note(format!("{name} got {text}"));
                                inbox.send(Msg::Agent(AgentEvent::Text(format!("{name}: {text}"))));
                                if text == "ask" {
                                    inbox.send(Msg::Agent(AgentEvent::PermissionRequest { detail: String::new(), id: 1, title: "Read".into(), choices: vec![PermissionChoice::new("allow", PermissionKind::AllowOnce)], edit: false, risk: magistr_domain::policy::RiskTier::Low, plan: None }));
                                }
                            }
                            Some(Cmd::Permission { id, option_id }) => {
                                log.note(format!("{name} answered {id} {option_id}"));
                                inbox.send(Msg::Agent(AgentEvent::PermissionCancelled { id }));
                            }
                            Some(Cmd::Cancel) => { log.note(format!("{name} cancelled")); }
                            Some(_) => {}
                            None => break,
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

#[tokio::test(flavor = "multi_thread")]
async fn addressed_sessions_in_two_workspaces_survive_disconnect_and_route_permissions() {
    let tmp = projects();
    let log = Log::default();
    let registry = registry(log.clone());
    let client = registry.client();
    let a = client.open(tmp.path().join("first")).await.unwrap();
    let b = client.open(tmp.path().join("second")).await.unwrap();
    let c = client.open(tmp.path().join("first")).await.unwrap();
    assert_ne!(a, c);
    assert_eq!(a.workspace, c.workspace);
    let mut reader = client.subscribe(&a).await.unwrap();
    client.command(&a, Cmd::Prompt("ask".into())).await.unwrap();
    until(&mut reader, |snapshot| snapshot.pending.contains_key(&1)).await;
    drop(reader);
    client.draft(&a, "unsent first".into()).await.unwrap();
    client
        .command(&b, Cmd::Prompt("other".into()))
        .await
        .unwrap();
    assert!(
        client
            .command(
                &b,
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
                &a,
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
            &a,
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
    client.command(&c, Cmd::Cancel).await.unwrap();
    assert!(
        client
            .open(tmp.path().join("held"))
            .await
            .unwrap_err()
            .contains("provider unavailable")
    );
    assert_eq!(client.list().await.unwrap().len(), 3);
    client.close(&a).await.unwrap();
    assert!(
        client
            .command(&a, Cmd::Cancel)
            .await
            .unwrap_err()
            .contains("no longer")
    );
    let mut forged = c.clone();
    forged.workspace = b.workspace.clone();
    assert!(client.command(&forged, Cmd::Cancel).await.is_err());
    client
        .command(&b, Cmd::Prompt("still alive".into()))
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
    client.open(tmp.path().join("first")).await.unwrap();
    client.open(tmp.path().join("broken-close")).await.unwrap();
    client.open(tmp.path().join("second")).await.unwrap();
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
    let a = client.open(tmp.path().join("first")).await.unwrap();
    let opening = {
        let client = client.clone();
        let root = tmp.path().join("second");
        tokio::spawn(async move { client.open(root).await })
    };
    tokio::task::yield_now().await;
    tokio::time::timeout(Duration::from_secs(1), client.command(&a, Cmd::Cancel))
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
    let client = SessionClient { sender };
    let (reply, _) = oneshot::channel();
    client.post(Request::List { reply }).unwrap();
    let address = SessionAddress {
        project: "p".into(),
        workspace: "w".into(),
        session: "s".into(),
    };
    assert!(
        client
            .command(&address, Cmd::Prompt("keep this draft".into()))
            .await
            .unwrap_err()
            .contains("busy")
    );
}

#[tokio::test]
async fn oversized_input_is_refused_before_entering_the_owner_queue() {
    let (sender, mut receiver) = mpsc::channel(1);
    let client = SessionClient { sender };
    let address = SessionAddress {
        project: "p".into(),
        workspace: "w".into(),
        session: "s".into(),
    };
    let text = "é".repeat(524_289);
    assert!(
        tokio::time::timeout(
            Duration::from_millis(100),
            client.draft(&address, text.clone())
        )
        .await
        .expect("must refuse before posting")
        .unwrap_err()
        .contains("too long")
    );
    assert!(
        tokio::time::timeout(
            Duration::from_millis(100),
            client.command(&address, Cmd::Prompt(text))
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
