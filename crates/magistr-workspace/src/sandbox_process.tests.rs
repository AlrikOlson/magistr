//! Process-level LSP transport and ownership regressions.

use super::*;

#[test]
fn fake_server_entry() {
    let Some(mode) =
        std::env::args().find_map(|arg| arg.strip_prefix("lsp-mode:").map(str::to_owned))
    else {
        return;
    };
    let mut out = std::io::stdout().lock();
    out.write_all(b"\n")
        .expect("separate libtest banner from transport");
    match mode.as_str() {
        "header" => out
            .write_all(&vec![b'x'; 16 * 1024])
            .expect("write long header"),
        "oversize" => out
            .write_all(b"Content-Length: 4194305\r\n\r\n")
            .expect("write oversize frame"),
        "malformed" => out
            .write_all(b"Content-Length: no\r\n\r\n")
            .expect("write malformed frame"),
        "flood" => {
            for _ in 0..128 {
                out.write_all(b"Content-Length: 1\r\n\r\nx")
                    .expect("write frame");
            }
        }
        "echo" => {
            out.flush().expect("flush startup");
            drop(out);
            let mut input = std::io::stdin().lock();
            let mut output = std::io::stdout().lock();
            let mut chunk = [0; 8192];
            loop {
                let read = input.read(&mut chunk).expect("read echo request");
                if read == 0 {
                    break;
                }
                output.write_all(&chunk[..read]).expect("echo frames");
                output.flush().expect("flush echo response");
            }
            return;
        }
        _ => {}
    }
    out.flush().expect("flush frames");
    drop(out);
    std::io::copy(&mut std::io::stdin().lock(), &mut std::io::sink())
        .expect("wait for ownership cleanup");
}

fn spec(mode: &str) -> SubprocessSpec {
    SubprocessSpec {
        language_id: mode.to_owned(),
        bin: std::env::current_exe()
            .expect("test executable")
            .to_string_lossy()
            .into_owned(),
        args: vec![
            "--exact".to_owned(),
            "sandbox::process_tests::fake_server_entry".to_owned(),
            format!("lsp-mode:{mode}"),
            "--nocapture".to_owned(),
            "--test-threads=1".to_owned(),
        ],
    }
}

#[test]
fn oversized_unterminated_lsp_header_is_rejected_before_body_allocation() {
    let mut pool = ServerPool::default();
    let session = pool.spawn(&spec("header"), None).expect("spawn");
    let result = session.frames.recv_timeout(Duration::from_secs(3));
    session.kill();
    assert!(
        matches!(result, Ok(Err(ref message)) if message.contains("header")),
        "bounded header must fail without waiting for EOF: {result:?}"
    );
}

#[test]
fn lsp_frame_queue_is_bounded_even_when_the_guest_does_not_read() {
    let mut pool = ServerPool::default();
    let session = pool.spawn(&spec("flood"), None).expect("spawn");
    let deadline = Instant::now() + Duration::from_secs(3);
    while !session.reader.as_ref().expect("owned reader").is_finished() && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(5));
    }
    let reader_finished = session.reader.as_ref().expect("owned reader").is_finished();
    let count = session.frames.try_iter().filter(Result::is_ok).count();
    session.kill();
    assert!(
        reader_finished,
        "saturated reader must stop without waiting for its guest"
    );
    assert!(
        count <= 16,
        "unread server output exceeded bounded queue: {count}"
    );
    assert!(count > 0, "fixture must produce real frames");
}

#[test]
fn global_lsp_session_limit_includes_private_invocation_sessions() {
    let mut pool = ServerPool::default();
    let mut sessions = Vec::new();
    for _ in 0..16 {
        sessions.push(pool.spawn(&spec("idle"), None).expect("within ceiling"));
    }
    let excess = pool.spawn(&spec("idle"), None);
    let refused = matches!(excess, Err(SubprocessOpenError::SpawnRefused(ref message)) if message.contains("ceiling"));
    if let Ok(session) = excess {
        session.kill();
    }
    for session in sessions {
        session.kill();
    }
    assert!(
        refused,
        "the process pool must enforce an aggregate live-child ceiling"
    );
    pool.spawn(&spec("idle"), None)
        .expect("cleanup returns capacity")
        .kill();
}

#[test]
fn malformed_and_oversize_lsp_frames_are_terminal_errors() {
    for mode in ["malformed", "oversize"] {
        let mut pool = ServerPool::default();
        let session = pool.spawn(&spec(mode), None).expect("spawn");
        let result = session.frames.recv_timeout(Duration::from_secs(3));
        session.kill();
        assert!(
            matches!(result, Ok(Err(_))),
            "{mode} frame must be refused: {result:?}"
        );
    }
}

#[test]
fn lsp_drop_reaps_only_the_session_child() {
    let mut pool = ServerPool::default();
    let mut unrelated = spawn_server_process(&spec("idle")).expect("unrelated child");
    let session = pool.spawn(&spec("idle"), None).expect("owned child");
    let pid = session.child.id();
    drop(session);
    let unrelated_alive = matches!(unrelated.try_wait(), Ok(None));
    let _ = unrelated.kill();
    let _ = unrelated.wait();
    assert!(unrelated_alive, "cleanup must only touch its owned child");
    #[cfg(unix)]
    assert!(
        !std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stderr(std::process::Stdio::null())
            .status()
            .expect("probe child liveness")
            .success(),
        "dropped session child must be reaped"
    );
    #[cfg(not(unix))]
    let _ = pid;
}

#[test]
fn language_spec_admission_and_per_invocation_ceiling_preserve_real_framing() {
    let mut specs = Vec::new();
    for index in 0..5 {
        let mut entry = spec("echo");
        entry.language_id = format!("language-{index}");
        specs.push(entry);
    }
    let mut view = SubprocessPool::new(specs, String::new(), false);
    assert!(matches!(
        view.open("not-granted"),
        Err(SubprocessOpenError::UnknownSpec(_))
    ));
    let first = view.open("language-0").expect("host spec accepted");
    let body = br#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#;
    view.write_frame(first, body)
        .expect("write real Content-Length frame");
    assert_eq!(
        view.read_frame(first, 3_000).expect("read echo frame"),
        body
    );
    for index in 1..4 {
        view.open(&format!("language-{index}"))
            .expect("within per-invoke ceiling");
    }
    assert!(matches!(
        view.open("language-4"),
        Err(SubprocessOpenError::SpawnRefused(_))
    ));
    view.settle(false);
    assert!(view.sessions.is_empty());
}

#[test]
fn warmed_lsp_shutdown_reaps_the_owned_process_and_recovers_capacity() {
    let mut pool = ServerPool::default();
    let launch = spec("idle");
    let key = PoolKey {
        root: String::new(),
        language_id: launch.language_id.clone(),
        bin: launch.bin.clone(),
        args: launch.args.clone(),
    };
    let session = pool
        .spawn(&launch, Some(key))
        .expect("spawn warm candidate");
    let pid = session.child.id();
    pool.return_warm(session);
    assert_eq!(pool.live_sessions.load(Ordering::SeqCst), 1);
    assert_eq!(pool.shutdown(), 1);
    assert_eq!(pool.live_sessions.load(Ordering::SeqCst), 0);
    assert!(pool.warm.is_empty());
    #[cfg(unix)]
    assert!(
        !std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stderr(std::process::Stdio::null())
            .status()
            .expect("probe child liveness")
            .success(),
        "shutdown must reap its child"
    );
    #[cfg(not(unix))]
    let _ = pid;
}
