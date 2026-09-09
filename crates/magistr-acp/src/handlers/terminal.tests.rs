use super::*;

fn events() -> (
    mpsc::UnboundedSender<Result<AcpEvent>>,
    mpsc::UnboundedReceiver<Result<AcpEvent>>,
) {
    mpsc::unbounded_channel()
}

fn create(
    mgr: &TerminalManager,
    tx: &mpsc::UnboundedSender<Result<AcpEvent>>,
    cmd: &str,
    args: &[&str],
) -> String {
    let req = CreateTerminalRequest::new("sess", cmd)
        .args(args.iter().map(|a| (*a).to_string()).collect());
    create_terminal(req, mgr, tx)
        .expect("spawn")
        .terminal_id
        .0
        .to_string()
}

#[tokio::test]
async fn runs_a_command_and_captures_its_output_and_exit_code() {
    let mgr = new_terminal_manager();
    let (tx, mut rx) = events();
    let tid = create(&mgr, &tx, "sh", &["-c", "echo out; echo err 1>&2; exit 3"]);

    let exit = wait_for_terminal_exit(&WaitForTerminalExitRequest::new("sess", tid.clone()), &mgr)
        .await
        .expect("wait");
    assert_eq!(exit.exit_status.exit_code, Some(3));

    // Readers may still be flushing the last bytes after exit.
    let mut output = None;
    for _ in 0..50 {
        let o = terminal_output(&TerminalOutputRequest::new("sess", tid.clone()), &mgr)
            .expect("output");
        if o.output.contains("out") && o.output.contains("err") {
            output = Some(o);
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let output = output.expect("both streams captured");
    assert!(!output.truncated);
    assert_eq!(
        output.exit_status.as_ref().and_then(|s| s.exit_code),
        Some(3)
    );

    assert!(matches!(
        rx.try_recv(),
        Ok(Ok(AcpEvent::System { subtype, .. })) if subtype == "terminal_create"
    ));
}

#[tokio::test]
async fn waiting_twice_returns_the_cached_status() {
    let mgr = new_terminal_manager();
    let (tx, _rx) = events();
    let tid = create(&mgr, &tx, "true", &[]);
    let req = WaitForTerminalExitRequest::new("sess", tid);
    let first = wait_for_terminal_exit(&req, &mgr).await.expect("first");
    let second = wait_for_terminal_exit(&req, &mgr).await.expect("second");
    assert_eq!(first.exit_status.exit_code, Some(0));
    assert_eq!(second.exit_status.exit_code, Some(0));
}

#[tokio::test]
async fn output_is_truncated_at_the_byte_limit() {
    let mgr = new_terminal_manager();
    let (tx, _rx) = events();
    let req = CreateTerminalRequest::new("sess", "sh")
        .args(vec!["-c".into(), "yes | head -c 20000".into()])
        .output_byte_limit(1024u64);
    let tid = create_terminal(req, &mgr, &tx)
        .expect("spawn")
        .terminal_id
        .0
        .to_string();
    wait_for_terminal_exit(&WaitForTerminalExitRequest::new("sess", tid.clone()), &mgr)
        .await
        .expect("wait");
    tokio::time::sleep(Duration::from_millis(50)).await;
    let out = terminal_output(&TerminalOutputRequest::new("sess", tid), &mgr).expect("output");
    assert!(out.truncated);
    assert!(out.output.len() < 20000);
}

#[tokio::test]
async fn kill_stops_a_running_command() {
    let mgr = new_terminal_manager();
    let (tx, _rx) = events();
    let tid = create(&mgr, &tx, "sleep", &["30"]);
    kill_terminal(&KillTerminalRequest::new("sess", tid.clone()), &mgr).expect("kill");
    let exit = tokio::time::timeout(
        Duration::from_secs(5),
        wait_for_terminal_exit(&WaitForTerminalExitRequest::new("sess", tid), &mgr),
    )
    .await
    .expect("killed promptly")
    .expect("wait");
    // Killed by a signal: no exit code.
    assert_eq!(exit.exit_status.exit_code, None);
}

#[tokio::test]
async fn release_kills_and_forbids_a_later_kill() {
    let mgr = new_terminal_manager();
    let (tx, mut rx) = events();
    let tid = create(&mgr, &tx, "sleep", &["30"]);
    release_terminal(&ReleaseTerminalRequest::new("sess", tid.clone()), &mgr, &tx)
        .expect("release");
    let err = kill_terminal(&KillTerminalRequest::new("sess", tid), &mgr).expect_err("released");
    assert_eq!(i32::from(err.code), INVALID_PARAMS);

    let subtypes: Vec<String> = std::iter::from_fn(|| rx.try_recv().ok())
        .filter_map(|e| match e {
            Ok(AcpEvent::System { subtype, .. }) => Some(subtype),
            _ => None,
        })
        .collect();
    assert_eq!(subtypes, vec!["terminal_create", "terminal_release"]);
}

#[tokio::test]
async fn unknown_terminal_ids_are_invalid_params() {
    let mgr = new_terminal_manager();
    let err =
        terminal_output(&TerminalOutputRequest::new("sess", "nope"), &mgr).expect_err("unknown");
    assert_eq!(i32::from(err.code), INVALID_PARAMS);
    let err = wait_for_terminal_exit(&WaitForTerminalExitRequest::new("sess", "nope"), &mgr)
        .await
        .expect_err("unknown");
    assert_eq!(i32::from(err.code), INVALID_PARAMS);
}

#[test]
fn a_missing_binary_is_an_internal_error() {
    let mgr = new_terminal_manager();
    let (tx, _rx) = events();
    let err = create_terminal(
        CreateTerminalRequest::new("sess", "/definitely/not/a/command"),
        &mgr,
        &tx,
    )
    .expect_err("spawn fails");
    assert_eq!(i32::from(err.code), INTERNAL_ERROR);
}

#[test]
fn terminal_ids_are_unique() {
    let a = next_terminal_id();
    let b = next_terminal_id();
    assert_ne!(a, b);
    assert!(a.starts_with("term-"));
}

/// Every `Terminal` chunk on the channel so far, joined in order.
fn streamed(rx: &mut mpsc::UnboundedReceiver<Result<AcpEvent>>, tid: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let Ok(AcpEvent::Terminal {
            terminal_id,
            bytes: chunk,
        }) = event
            && terminal_id == tid
        {
            bytes.extend_from_slice(&chunk);
        }
    }
    bytes
}

#[cfg(unix)]
#[tokio::test]
async fn on_a_pty_the_bytes_stream_raw_and_the_agent_reads_stripped_text() {
    let mgr = new_terminal_manager();
    let (tx, mut rx) = events();
    let tid = create(
        &mgr,
        &tx,
        "sh",
        &[
            "-c",
            "test -t 1 && printf '\\033[31mred\\033[0m\\n'; printf 'done\\n'",
        ],
    );
    let exit = wait_for_terminal_exit(&WaitForTerminalExitRequest::new("sess", tid.clone()), &mgr)
        .await
        .expect("wait");
    assert_eq!(exit.exit_status.exit_code, Some(0), "stdout is a tty");

    // The wait drained the reader: the output is whole, no polling needed.
    let out =
        terminal_output(&TerminalOutputRequest::new("sess", tid.clone()), &mgr).expect("output");
    assert_eq!(out.output, "red\ndone\n");

    match rx.try_recv() {
        Ok(Ok(AcpEvent::System { subtype, data })) => {
            assert_eq!(subtype, "terminal_create");
            assert_eq!(data["pty"], true);
            assert_eq!(data["pane"], "live");
            assert!(data["reason"].is_null());
        }
        other => panic!("expected terminal_create first, got {other:?}"),
    }
    assert_eq!(streamed(&mut rx, &tid), b"\x1b[31mred\x1b[0m\r\ndone\r\n");
}

#[tokio::test]
async fn the_output_streams_as_terminal_events_in_order() {
    let mgr = new_terminal_manager();
    let (tx, mut rx) = events();
    let tid = create(
        &mgr,
        &tx,
        "sh",
        &[
            "-c",
            "i=0; while [ $i -lt 200 ]; do echo line$i; i=$((i+1)); done",
        ],
    );
    wait_for_terminal_exit(&WaitForTerminalExitRequest::new("sess", tid.clone()), &mgr)
        .await
        .expect("wait");
    let bytes = streamed(&mut rx, &tid);
    let text = String::from_utf8_lossy(&bytes).replace("\r\n", "\n");
    let expected: String = (0..200).map(|i| format!("line{i}\n")).collect();
    assert_eq!(text, expected);
    let out = terminal_output(&TerminalOutputRequest::new("sess", tid), &mgr).expect("output");
    assert_eq!(out.output, expected);
}
