use super::*;
use magistr_application::effects::SourceRoute;
use magistr_application::sessions::AgentEvent;
use magistr_cockpit::cockpit::Msg;
use magistr_engine::NativeConfig;
use magistr_workspace::service::WorkspaceOpenOptions;
use std::pin::Pin;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::{Notify, mpsc};

async fn fixture() -> (tempfile::TempDir, AgentConfig, NativeConfig, Resources) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let inputs = crate::config::inputs::ProjectInputs::from_lookup(&root, |_| None);
    let config = AgentConfig::new("", &root).with_inputs(&inputs);
    let native = magistr_core::evidence::writer_config(&root, |_| None)
        .with_data_dir(dir.path().join("store"));
    let engines = EngineClient::with_config(native.clone()).unwrap();
    let endpoint = engines.serve_owner().await.unwrap();
    // An empty tree needs no parser worker; this exercises actual admission and shutdown.
    let workspace = WorkspaceAdmission::admit_with(
        WorkspaceOpenOptions::new(&root, std::env::current_exe().unwrap())
            .with_network_override("0"),
    )
    .await;
    assert!(workspace.hosted().is_some(), "{}", workspace.report());
    (
        dir,
        config,
        native,
        Resources {
            admitted: Arc::new(AdmittedResources {
                endpoint,
                workspace,
            }),
            engines,
        },
    )
}

fn resolved(config: &AgentConfig) -> crate::error::Result<ResolvedSessionSpec> {
    ResolvedSessionSpec::resolve_with_runtime(
        config.clone(),
        &crate::config::user_config::UserConfig::default(),
        crate::agent::AgentRuntime {
            path: "not-spawned".into(),
            args: vec![],
            auth: crate::agent::AuthMethod::Implicit,
            provider: "claude".into(),
        },
    )
}

#[derive(Clone, Default)]
struct Probe {
    closing: Arc<Notify>,
    finish: Arc<Notify>,
    dropped: Arc<AtomicBool>,
}

struct Session {
    probe: Probe,
    conversation: magistr_application::sessions::Conversation,
    _source: SourceRoute,
    tx: mpsc::UnboundedSender<AgentEvent>,
    rx: mpsc::UnboundedReceiver<AgentEvent>,
}

impl Drop for Session {
    fn drop(&mut self) {
        self.probe.dropped.store(true, Ordering::SeqCst);
    }
}

impl SessionHandle for Session {
    fn conversation(&self) -> Option<magistr_application::sessions::Conversation> {
        Some(self.conversation.clone())
    }
    fn submit_turn(
        &mut self,
        input: &magistr_application::sessions::PromptInput,
    ) -> Result<(), String> {
        self.tx.send(AgentEvent::Text(input.text.clone())).unwrap();
        Ok(())
    }
    fn rename(&mut self, _: &str) -> Result<(), String> {
        Err("Naming is unavailable in this session.".into())
    }
    fn configure(&mut self, _: &str, _: &str) -> Result<(), String> {
        Err("Agent choices are unavailable in this session.".into())
    }
    fn cancel_turn(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn answer_permission(&mut self, _: u64, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn constrain_permission(&mut self, _: u64, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn delegate_permission(&mut self, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn kill_terminal(&mut self, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn detach_client(&mut self) {}
    fn next_event(&mut self) -> Pin<Box<dyn Future<Output = Option<AgentEvent>> + Send + '_>> {
        Box::pin(self.rx.recv())
    }
    fn terminate_session(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
        Box::pin(async {
            self.probe.closing.notify_one();
            self.probe.finish.notified().await;
            Ok(())
        })
    }
}

async fn started(
    config: AgentConfig,
    resources: Resources,
    probe: Probe,
) -> (
    InteractiveHost,
    mpsc::UnboundedSender<Cmd>,
    mpsc::UnboundedReceiver<Msg>,
) {
    let (tx, rx) = mpsc::unbounded_channel();
    let session = Session {
        probe,
        conversation: magistr_application::sessions::Conversation {
            id: config.history.id(),
            purpose: config.history.purpose(),
            name: None,
            provider: "claude".into(),
        },
        _source: resources.admitted.workspace.source_route(),
        tx,
        rx,
    };
    let spec = resolved(&config);
    let (inbox, events) = Inbox::channel();
    let (sender, commands) = Commands::channel();
    let root = config.cwd.canonicalize().unwrap().display().to_string();
    let (host, seed) = InteractiveHost::connect(
        config,
        resources,
        spec,
        inbox,
        commands,
        |_, _| async { Ok(Box::new(session) as Box<dyn SessionHandle>) },
        Connector::live,
        None,
    )
    .await
    .unwrap();
    assert_eq!(seed.name, "project");
    assert_eq!(seed.dir, root);
    (host, sender, events)
}

async fn wait_for_release(native: &NativeConfig) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Ok(client) = EngineClient::with_config(native.clone()) {
                drop(client);
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("host released");
}

#[tokio::test(flavor = "multi_thread")]
async fn startup_failure_releases_source_and_project_owners() {
    for resolution_fails in [true, false] {
        let (_dir, config, native, resources) = fixture().await;
        let spec = if resolution_fails {
            Err(crate::error::MagistrError::Config(
                "invalid selection".into(),
            ))
        } else {
            resolved(&config)
        };
        let (inbox, _events) = Inbox::channel();
        let (_sender, commands) = Commands::channel();
        let result = InteractiveHost::connect(
            config,
            resources,
            spec,
            inbox,
            commands,
            |_, _| async { Err(eyre::eyre!("provider unavailable")) },
            Connector::live,
            None,
        )
        .await;
        let error = result.err().expect("startup refused");
        assert_eq!(error.to_string(), "Magistr could not start the session");
        wait_for_release(&native).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn dropping_a_host_closes_its_session_before_releasing_the_project() {
    let (_dir, config, native, resources) = fixture().await;
    let probe = Probe::default();
    let (host, sender, mut events) = started(config, resources, probe.clone()).await;
    sender.send(Cmd::Prompt("Still working".into())).unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let event = events.recv().await.expect("session events remain open");
            if matches!(event, Msg::Agent(AgentEvent::Text(text)) if text == "Still working") {
                break;
            }
        }
    })
    .await
    .unwrap();
    drop(host);
    tokio::time::timeout(Duration::from_secs(2), probe.closing.notified())
        .await
        .unwrap();
    assert!(
        EngineClient::with_config(native.clone()).is_err(),
        "owner stays during close"
    );
    assert!(sender.send(Cmd::Prompt("Too late".into())).is_err());
    probe.finish.notify_one();
    wait_for_release(&native).await;
    assert!(probe.dropped.load(Ordering::SeqCst));
}

#[tokio::test(flavor = "multi_thread")]
async fn unreadable_work_records_are_reported_without_preventing_chat_startup() {
    use magistr_engine::{ReadErrorKind, ReadState};
    use magistr_workflow::infra::{Domain, Persistence};

    let (_dir, config, native, resources) = fixture().await;
    let path = Persistence::new(&native.persistence_config(), Domain::Roadmap)
        .path_for(resources.engines.reader().project_id());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"not valid records").unwrap();
    let probe = Probe::default();
    let (tx, rx) = mpsc::unbounded_channel();
    let session = Session {
        probe: probe.clone(),
        conversation: magistr_application::sessions::Conversation {
            id: config.history.id(),
            purpose: config.history.purpose(),
            name: None,
            provider: "claude".into(),
        },
        _source: resources.admitted.workspace.source_route(),
        tx,
        rx,
    };
    let spec = resolved(&config);
    let (inbox, mut events) = Inbox::channel();
    let (sender, commands) = Commands::channel();
    let (host, seed) = InteractiveHost::connect(
        config,
        resources,
        spec,
        inbox,
        commands,
        |_, _| async { Ok(Box::new(session) as Box<dyn SessionHandle>) },
        Connector::live,
        None,
    )
    .await
    .unwrap();
    assert!(matches!(seed.snapshot.roadmap, ReadState::Unavailable(_)));
    assert_eq!(
        seed.snapshot.roadmap.current().unwrap_err().kind,
        ReadErrorKind::Decode
    );
    assert!(seed.snapshot.ship.current().is_ok());
    sender.send(Cmd::Prompt("Chat still works".into())).unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let event = events.recv().await.expect("session remains open");
            if matches!(event, Msg::Agent(AgentEvent::Text(text)) if text == "Chat still works") {
                break;
            }
        }
    })
    .await
    .unwrap();
    probe.finish.notify_one();
    host.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn explicit_shutdown_works_while_the_surface_keeps_its_sender() {
    let (_dir, config, native, resources) = fixture().await;
    let probe = Probe::default();
    let (host, sender, _events) = started(config, resources, probe.clone()).await;
    probe.finish.notify_one();
    host.shutdown().await.unwrap();
    assert!(probe.dropped.load(Ordering::SeqCst));
    assert!(sender.is_closed());
    EngineClient::with_config(native).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn later_sessions_use_the_model_accepted_by_the_opening_provider() {
    use magistr_acp::{AcpTransport, test_support::FakeAcpAgent};

    let (_dir, config, _native, resources) = fixture().await;
    let mut defaults = crate::config::user_config::UserConfig::default();
    defaults.models.claude = Some("unavailable-preference".into());
    let spec = ResolvedSessionSpec::resolve_with_runtime(
        config.clone(),
        &defaults,
        crate::agent::AgentRuntime {
            path: "not-spawned".into(),
            args: vec![],
            auth: crate::agent::AuthMethod::Implicit,
            provider: "claude".into(),
        },
    );
    let history_inputs = config.inputs().as_ref().clone();
    let (chosen, mut choices) = mpsc::unbounded_channel();
    let (inbox, _events) = Inbox::channel();
    let (sender, commands) = Commands::channel();
    let (host, _) = InteractiveHost::connect(
        config,
        resources,
        spec,
        inbox,
        commands,
        |spec, connector| async move {
            spec.history()
                .begin(&history_inputs, spec.cwd(), "claude", spec.model())
                .await?;
            let (channel, _, agent) = FakeAcpAgent::new().spawn();
            let mut transport = AcpTransport::new_with_args("not-spawned".into(), vec![], None)?
                .session_model_preferred(spec.model().map(str::to_owned));
            transport.connect_over(channel).await?;
            assert_eq!(transport.session_config_options().unwrap().len(), 1);
            let session = connector
                .start_session(
                    transport,
                    spec,
                    magistr_application::sessions::SessionOptions::new(),
                )
                .await?;
            drop(agent);
            Ok(Box::new(session) as Box<dyn SessionHandle>)
        },
        move |_| {
            Connector::new(move |config| {
                chosen
                    .send(config.inputs().session_selection().cloned())
                    .unwrap();
                async {
                    Err(crate::error::MagistrError::Other(
                        "selection captured".into(),
                    ))
                }
            })
        },
        None,
    )
    .await
    .unwrap();

    sender
        .send(Cmd::RoadmapPrompt("Plan the next step".into()))
        .unwrap();
    let selected = tokio::time::timeout(Duration::from_secs(5), choices.recv())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(selected.agent, "claude");
    assert_eq!(
        selected.model, None,
        "the provider accepted its default, not the unavailable preference"
    );
    assert!(
        !sender.is_closed(),
        "a refused planning connection keeps the host usable"
    );
    host.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_host_closes_its_observation_and_keeps_the_draft() {
    use crate::application::session_observation::{SessionObservation, SessionStatus};
    use magistr_cockpit_runtime::channel::ObservationSink;

    let (_dir, config, native, resources) = fixture().await;
    let observation = Arc::new(SessionObservation::new(
        magistr_domain::identity::SessionAddress {
            project: "project".into(),
            workspace: config.cwd.display().to_string(),
            session: "resident".into(),
        },
    ));
    let conversation = magistr_application::sessions::Conversation {
        id: config.history.id(),
        purpose: config.history.purpose(),
        name: None,
        provider: "claude".into(),
    };
    observation.bind_context_for_test(conversation.clone());
    observation.activate_context(&conversation.id).unwrap();
    observation
        .draft_for(&conversation.id, "Keep this unsent message".into())
        .unwrap();
    let (tx, rx) = mpsc::unbounded_channel();
    let probe = Probe::default();
    probe.finish.notify_one();
    let session = Session {
        probe,
        conversation: conversation.clone(),
        _source: resources.admitted.workspace.source_route(),
        tx,
        rx,
    };
    let spec = resolved(&config);
    let (sender, commands) = Commands::channel();
    let (host, _) = InteractiveHost::connect(
        config,
        resources,
        spec,
        Inbox::observed(observation.clone()),
        commands,
        |_, _| async { Ok(Box::new(session) as Box<dyn SessionHandle>) },
        |_| Connector::new(|_| async { panic!("controlled host failure") }),
        Some(observation.clone()),
    )
    .await
    .unwrap();
    sender
        .send(Cmd::RoadmapPrompt("Open planning".into()))
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), observation.closed())
        .await
        .unwrap();
    let snapshot = observation.snapshot();
    assert!(snapshot.closed);
    assert_eq!(snapshot.status, SessionStatus::Failed);
    assert_eq!(snapshot.conversation.as_ref().unwrap().id, conversation.id);
    assert_eq!(
        snapshot.seed.draft.as_deref(),
        Some("Keep this unsent message")
    );
    assert!(snapshot.replay().iter().any(|message| {
        matches!(message, Msg::Agent(AgentEvent::Error(error)) if error.contains("controlled host failure"))
    }));
    assert!(sender.is_closed());
    assert!(
        host.shutdown()
            .await
            .unwrap_err()
            .contains("controlled host failure")
    );
    wait_for_release(&native).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_host_attaches_workflow_and_refuses_source_without_replacing_owners() {
    let (_dir, config, native, first) = fixture().await;
    let second = EngineClient::admit_or_attach_config(native.clone()).unwrap();
    assert!(second.writer().is_none());
    assert!(second.attached_generation().is_some());
    let resources = Resources::open(second, config.inputs()).await.unwrap();
    assert!(resources.admitted.endpoint.is_none());
    assert!(matches!(
        resources.admitted.workspace.source_route(),
        SourceRoute::Refused(magistr_application::effects::SourceRefusal::OwnerElsewhere)
    ));
    resources.close().await.unwrap();
    assert!(EngineClient::with_config(native.clone()).is_err());
    assert!(first.admitted.workspace.hosted().is_some());
    first.close().await.unwrap();
    EngineClient::with_config(native).unwrap();
}

#[tokio::test]
async fn close_deadline_aborts_and_joins_an_unresponsive_relay() {
    let dropped = Arc::new(AtomicBool::new(false));
    struct Mark(Arc<AtomicBool>);
    impl Drop for Mark {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let mark = Mark(dropped.clone());
    let relay = OwnedTask(tokio::spawn(async move {
        let _mark = mark;
        std::future::pending::<Result<(), String>>().await
    }));
    let (_sender, commands) = Commands::channel();
    let (forward, _receiver) = Commands::bounded(32);
    let (stop, stopped) = oneshot::channel();
    stop.send(()).unwrap();
    let (inbox, _events) = Inbox::channel();
    let error = serve(commands, forward, stopped, relay, inbox)
        .await
        .unwrap_err();
    assert!(error.contains("too long"));
    assert!(dropped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn a_closed_relay_returns_the_undelivered_command_to_the_surface() {
    let finish = Arc::new(Notify::new());
    let release = finish.clone();
    let relay = OwnedTask(tokio::spawn(async move {
        release.notified().await;
        Ok(())
    }));
    let (sender, commands) = Commands::channel();
    let (forward, receiver) = Commands::bounded(32);
    drop(receiver);
    let (_stop, stopped) = oneshot::channel();
    let (inbox, mut events) = Inbox::channel();
    let task = tokio::spawn(serve(commands, forward, stopped, relay, inbox));
    sender.send(Cmd::Prompt("Keep this draft".into())).unwrap();
    let event = tokio::time::timeout(Duration::from_secs(2), events.recv())
        .await
        .unwrap();
    assert!(
        matches!(event, Some(Msg::CommandFailed { cmd: Cmd::Prompt(text), error })
        if text.text == "Keep this draft" && error.contains("closed"))
    );
    finish.notify_one();
    assert!(task.await.unwrap().unwrap_err().contains("session ended"));
}
#[tokio::test(flavor = "multi_thread")]
async fn same_copy_sessions_share_source_owner_and_keep_separate_workflow_executions() {
    let (_dir, config, native, resources) = fixture().await;
    let writer = resources.engines.writer().unwrap();
    let first = writer.for_working_copy(&config.cwd, "first").unwrap();
    let second = writer.for_working_copy(&config.cwd, "second").unwrap();
    first
        .ship_set_objective("First".into(), vec![], vec![], "chunk:first".into())
        .unwrap();
    second
        .ship_set_objective("Second".into(), vec![], vec![], "chunk:second".into())
        .unwrap();
    assert_eq!(
        first.ship_objective().unwrap().unwrap().scope,
        "chunk:first"
    );
    assert_eq!(
        second.ship_objective().unwrap().unwrap().scope,
        "chunk:second"
    );
    let first_resources = resources.fork(EngineClient::attach(first));
    let second_resources = resources.fork(EngineClient::attach(second));
    assert!(Arc::ptr_eq(
        &first_resources.admitted,
        &second_resources.admitted
    ));
    let SourceRoute::Admitted(first_source) = first_resources.admitted.workspace.source_route()
    else {
        panic!("first source");
    };
    let SourceRoute::Admitted(second_source) = second_resources.admitted.workspace.source_route()
    else {
        panic!("second source");
    };
    let root = config.cwd.canonicalize().unwrap();
    let first_path = root.join("first.txt").display().to_string();
    let second_path = root.join("second.txt").display().to_string();
    first_source.write_text(&first_path, "first").await.unwrap();
    second_source
        .write_text(&second_path, "second")
        .await
        .unwrap();
    assert_eq!(std::fs::read_to_string(&first_path).unwrap(), "first");
    assert_eq!(std::fs::read_to_string(&second_path).unwrap(), "second");
    drop(first_source);
    drop(second_source);
    first_resources.close().await.unwrap();
    assert!(resources.admitted.workspace.hosted().is_some());
    second_resources.close().await.unwrap();
    resources.close_last().await.unwrap();
    EngineClient::with_config(native).unwrap();
}
