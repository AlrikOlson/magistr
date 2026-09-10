use super::*;

fn unconnected() -> AcpTransport {
    AcpTransport::new_with_args(PathBuf::from("/usr/bin/magistr"), vec![], None)
        .expect("constructing never fails")
}

#[test]
fn not_ready_before_connect() {
    let t = unconnected();
    assert!(!t.is_ready());
    assert_eq!(t.child_pgid(), 0);
}

#[test]
fn read_events_is_none_before_connect() {
    let mut t = unconnected();
    assert!(t.read_events().is_none());
    assert!(t.take_permission_rx().is_none());
}

#[test]
fn prompt_before_connect_is_an_error() {
    let t = unconnected();
    let err = t.prompt("hi").expect_err("not connected");
    assert!(matches!(err, AcpError::Transport(m) if m.contains("not ready")));
}

#[test]
fn cancel_before_connect_is_an_error() {
    let t = unconnected();
    assert!(t.cancel().is_err());
}

#[tokio::test]
async fn close_without_connect_is_ok() {
    let mut t = unconnected();
    assert!(t.close().await.is_ok());
}

struct TaskDropped(Arc<AtomicBool>);

impl Drop for TaskDropped {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

async fn stuck_transport() -> (AcpTransport, Arc<AtomicBool>) {
    let dropped = Arc::new(AtomicBool::new(false));
    let guard = TaskDropped(Arc::clone(&dropped));
    let (started_tx, started_rx) = oneshot::channel();
    let mut transport = unconnected();
    transport.ready.store(true, Ordering::SeqCst);
    transport.connection = Some(tokio::spawn(async move {
        let _guard = guard;
        started_tx.send(()).expect("waiting for the task");
        std::future::pending::<()>().await;
    }));
    started_rx.await.expect("connection started");
    (transport, dropped)
}

#[tokio::test(start_paused = true)]
async fn close_timeout_aborts_and_joins_the_connection_before_returning() {
    let (mut transport, dropped) = stuck_transport().await;

    let outcome = transport.close().await;

    assert!(dropped.load(Ordering::SeqCst), "connection must be dropped");
    assert!(matches!(outcome, Err(AcpError::Timeout(_))));
    assert!(!transport.is_ready());
    assert!(transport.connection.is_none());
    transport.close().await.expect("close remains idempotent");
}

#[tokio::test(start_paused = true)]
async fn cancelled_close_preserves_the_connection_for_drop_cleanup() {
    let (mut transport, dropped) = stuck_transport().await;

    assert!(
        tokio::time::timeout(Duration::from_millis(1), transport.close())
            .await
            .is_err()
    );
    assert!(
        transport.connection.is_some(),
        "close must retain ownership"
    );
    drop(transport);
    tokio::task::yield_now().await;
    assert!(dropped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn close_reports_connection_task_failure() {
    let mut transport = unconnected();
    transport.connection = Some(tokio::spawn(async { panic!("connection failed") }));

    let error = transport.close().await.expect_err("task failed");

    assert!(matches!(error, AcpError::Transport(message) if message.contains("connection failed")));
}

#[tokio::test]
async fn cancelled_handshake_keeps_the_connection_owned_until_drop() {
    let (client, _silent_agent) = agent_client_protocol::Channel::duplex();
    let mut transport = unconnected();
    assert!(
        tokio::time::timeout(Duration::from_millis(100), transport.connect_over(client))
            .await
            .is_err()
    );
    let task = transport
        .connection
        .as_ref()
        .expect("handshake retains its connection task")
        .abort_handle();
    drop(transport);
    tokio::task::yield_now().await;
    assert!(task.is_finished(), "drop must abort the pending handshake");
}

#[tokio::test]
async fn cancelling_connect_as_handshake_finishes_keeps_a_usable_session() {
    use agent_client_protocol::schema::ProtocolVersion;
    use agent_client_protocol::schema::v1::{
        InitializeRequest, InitializeResponse, NewSessionRequest, NewSessionResponse,
    };
    use agent_client_protocol::{Agent, Channel, on_receive_request};

    let (client, server) = Channel::duplex();
    let agent = tokio::spawn(
        Agent
            .builder()
            .on_receive_request(
                async |_: InitializeRequest, responder, _| {
                    responder.respond(InitializeResponse::new(ProtocolVersion::V1))
                },
                on_receive_request!(),
            )
            .on_receive_request(
                async |_: NewSessionRequest, responder, _| {
                    responder.respond(NewSessionResponse::new("ready-session"))
                },
                on_receive_request!(),
            )
            .connect_to(server),
    );
    let mut transport = unconnected();
    let ready = Arc::clone(&transport.ready);
    tokio::time::timeout(Duration::from_secs(5), async {
        tokio::select! {
            biased;
            () = async {
                while !ready.load(Ordering::SeqCst) {
                    tokio::task::yield_now().await;
                }
            } => {}
            result = transport.connect_over(client) => {
                panic!("connect should be cancelled as the handshake completes: {result:?}");
            }
        }
    })
    .await
    .expect("handshake completes");

    assert!(transport.is_ready());
    transport
        .sender()
        .expect("ready session must have a sender");
    let mut events = transport.read_events().expect("ready session has events");
    assert!(matches!(
        events.try_recv(),
        Ok(Ok(AcpEvent::Initialized { .. }))
    ));
    assert!(transport.take_permission_rx().is_some());
    transport
        .close()
        .await
        .expect("close the completed handshake");
    agent
        .await
        .expect("agent task joined")
        .expect("agent closed");
}

#[tokio::test]
async fn reconnect_keeps_the_previous_connection_owned_until_cleanup() {
    let (mut transport, dropped) = stuck_transport().await;
    transport.ready.store(false, Ordering::SeqCst);
    let (client, _silent_agent) = agent_client_protocol::Channel::duplex();
    assert!(
        tokio::time::timeout(Duration::from_millis(100), transport.connect_over(client))
            .await
            .is_err()
    );

    drop(transport);
    tokio::task::yield_now().await;

    assert!(
        dropped.load(Ordering::SeqCst),
        "reconnect must not replace an owned task"
    );
}

#[tokio::test]
async fn close_stops_event_relays_even_when_the_source_remains_open() {
    let mut transport = unconnected();
    let (source, events) = mpsc::unbounded_channel();
    transport.event_rx = Some(events);
    let mut tap = transport.tap_events().expect("tap");
    let mut events = transport.read_events().expect("forwarded events");

    transport.close().await.expect("close");

    assert!(source.is_closed(), "relay must release its source receiver");
    assert!(tap.recv().await.is_none());
    assert!(events.recv().await.is_none());
}

#[cfg(unix)]
async fn stuck_subprocess_transport() -> (AcpTransport, i32, Arc<AtomicBool>) {
    let mut transport = unconnected();
    let args = vec!["-c".to_owned(), "exec sleep 600".to_owned()];
    let spawned = spawn::spawn_agent(spawn::build_command(
        std::path::Path::new("sh"),
        &args,
        None,
    ))
    .expect("spawn sleep in the agent's process group");
    let pid = spawned.child.id().expect("live child") as i32;
    transport.child_pgid.store(pid, Ordering::SeqCst);
    transport
        .background_tasks
        .push(drain_stderr(spawned.stderr, transport.stderr_tail.clone()));
    let dropped = Arc::new(AtomicBool::new(false));
    let guard = TaskDropped(Arc::clone(&dropped));
    let (started, running) = oneshot::channel();
    transport.connection = Some(tokio::spawn(async move {
        let _child = spawned.child;
        let _guard = guard;
        started.send(()).expect("waiting for connection");
        std::future::pending::<()>().await;
    }));
    running.await.expect("child belongs to connection task");
    (transport, pid, dropped)
}

#[cfg(unix)]
async fn assert_subprocess_reaped(pid: i32) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            // SAFETY: signal zero only checks whether this known child exists.
            if unsafe { libc::kill(pid, 0) } == -1 {
                assert_eq!(
                    std::io::Error::last_os_error().raw_os_error(),
                    Some(libc::ESRCH)
                );
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("agent process must exit and be reaped");
}

#[cfg(unix)]
#[tokio::test(start_paused = true)]
async fn close_timeout_cleans_the_owned_subprocess_and_stderr_task() {
    let (mut transport, pid, dropped) = stuck_subprocess_transport().await;

    assert!(matches!(transport.close().await, Err(AcpError::Timeout(_))));
    assert!(dropped.load(Ordering::SeqCst));
    assert!(transport.background_tasks.is_empty());
    assert_eq!(transport.child_pgid(), 0);
    tokio::time::resume();
    assert_subprocess_reaped(pid).await;
}

#[cfg(unix)]
#[tokio::test]
async fn drop_cleans_the_owned_subprocess_connection_and_stderr_task() {
    let (transport, pid, dropped) = stuck_subprocess_transport().await;
    let stderr_task = transport.background_tasks[0].abort_handle();

    drop(transport);

    assert_subprocess_reaped(pid).await;
    assert!(dropped.load(Ordering::SeqCst));
    assert!(stderr_task.is_finished());
}

#[tokio::test]
async fn connect_to_a_missing_binary_fails_fast() {
    let mut t =
        AcpTransport::new_with_args(PathBuf::from("/definitely/not/an/agent"), vec![], None)
            .expect("construct");
    let err = t.connect().await.expect_err("spawn must fail");
    assert!(matches!(err, AcpError::Connection(m) if m.contains("Failed to spawn")));
    assert!(!t.is_ready());
}

#[tokio::test]
async fn an_agent_that_exits_without_speaking_is_reported_with_its_exit_code() {
    // `sh -c 'exit 1'` exits 1 immediately without ever writing to stdout
    // — on Windows too, where Git puts an `sh` on PATH and `false` is not
    // a thing.
    let mut t = AcpTransport::new_with_args(
        PathBuf::from("sh"),
        vec!["-c".to_owned(), "exit 1".to_owned()],
        None,
    )
    .expect("construct");
    let err = t.connect().await.expect_err("handshake must fail");
    // Three truthful wordings race: the child watcher saw the exit, the SDK
    // failed the pending request on EOF, or the transport component tore the
    // whole connection down before either. Any of them is the right report.
    let msg = err.to_string();
    assert!(
        msg.contains("exited during initialize")
            || msg.contains("closed")
            || msg.contains("before the agent handshake completed"),
        "got: {msg}"
    );
    assert!(!t.is_ready());
}

/// The cockpit's held kill reaches only the configured application port.
#[test]
fn kill_terminal_delegates_to_the_connected_capability() {
    use magistr_application::effects::{
        EffectAuthorizer, EffectCapabilities, EffectError, EffectFuture, EffectRequest,
        EffectResponse,
    };
    use std::sync::Mutex;

    #[derive(Default)]
    struct Port {
        killed: Mutex<Vec<String>>,
    }
    impl EffectPort for Port {
        fn capabilities(&self) -> EffectCapabilities {
            EffectCapabilities::default()
        }
        fn execute(
            &self,
            _: EffectRequest,
            _: Arc<dyn EffectAuthorizer>,
        ) -> EffectFuture<'_, EffectResponse> {
            Box::pin(async { Err(EffectError::Unavailable("not part of this test".into())) })
        }
        fn kill_terminal(&self, id: &str) -> std::result::Result<(), EffectError> {
            self.killed.lock().expect("lock").push(id.to_owned());
            Ok(())
        }
        fn close(&self) {}
    }
    let mut t = unconnected();
    let err = t.kill_terminal("nobody").expect_err("no capability");
    assert!(
        matches!(&err, AcpError::Transport(m) if m.contains("cannot kill terminal nobody")),
        "{err:?}"
    );
    let port = Arc::new(Port::default());
    let (events, _event_rx) = mpsc::unbounded_channel();
    let (permissions, _permission_rx) = mpsc::unbounded_channel();
    t.effects = Some(Arc::new(EffectBridge::new(
        Some(port.clone()),
        permissions,
        events,
    )));
    t.kill_terminal("owned").expect("delegated");
    assert_eq!(*port.killed.lock().expect("lock"), ["owned"]);
}
