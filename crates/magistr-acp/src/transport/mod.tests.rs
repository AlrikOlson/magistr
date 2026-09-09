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

/// The cockpit's held kill reaches the child `terminal/create` spawned
/// through the handle — the manager is one, shared with the connection
/// — and does exactly what the agent's own `terminal/kill` does: the
/// child ends, the terminal stays valid for its exit status.
#[tokio::test]
async fn kill_terminal_ends_a_created_terminals_child_and_names_an_unknown_one() {
    use agent_client_protocol::schema::v1::{CreateTerminalRequest, WaitForTerminalExitRequest};
    let t = unconnected();
    let err = t.kill_terminal("nobody").expect_err("no such terminal");
    assert!(
        matches!(&err, AcpError::Transport(m) if m.contains("cannot kill terminal nobody")),
        "{err:?}"
    );
    let (tx, _rx) = mpsc::unbounded_channel();
    let req = CreateTerminalRequest::new("sess", "sleep").args(vec!["30".to_owned()]);
    let tid = crate::handlers::terminal::create_terminal(req, &t.terminals, &tx)
        .expect("spawn")
        .terminal_id
        .0
        .to_string();
    t.kill_terminal(&tid).expect("killed");
    let exit = tokio::time::timeout(
        Duration::from_secs(5),
        crate::handlers::terminal::wait_for_terminal_exit(
            &WaitForTerminalExitRequest::new("sess", tid.clone()),
            &t.terminals,
        ),
    )
    .await
    .expect("the child ended promptly")
    .expect("still a valid terminal");
    assert_eq!(
        exit.exit_status.exit_code, None,
        "a killed command has no exit code — the handler's own reading of a kill"
    );
}
