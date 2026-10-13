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
    fn submit_turn(&mut self, text: &str) -> Result<(), String> {
        self.tx.send(AgentEvent::Text(text.into())).unwrap();
        Ok(())
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
        |_| async { Ok(Box::new(session) as Box<dyn SessionHandle>) },
        Connector::live,
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
            |_| async { Err(eyre::eyre!("provider unavailable")) },
            Connector::live,
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
        if text == "Keep this draft" && error.contains("closed"))
    );
    finish.notify_one();
    task.await.unwrap().unwrap();
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
    assert_eq!(first.ship_status()["objective"]["scope"], "chunk:first");
    assert_eq!(second.ship_status()["objective"]["scope"], "chunk:second");
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
