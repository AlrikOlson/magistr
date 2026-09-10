//! The transport against a scripted in-process agent: handshake, streaming,
//! the agent's requests back to the client, cancel, config, and going away.

mod fake_agent;

use std::path::PathBuf;
use std::time::Duration;

use agent_client_protocol::schema::v1::{
    Meta, RequestPermissionOutcome, SessionConfigOptionValue, SessionUpdate, StopReason,
};
use fake_agent::{FABLE, SESSION_ID, Script};
use magistr_acp::{AcpError, AcpEvent, AcpPermissionResponse, AcpSessionMode, AcpTransport};
use tokio::sync::mpsc;

type Events = mpsc::UnboundedReceiver<magistr_acp::Result<AcpEvent>>;

const WAIT: Duration = Duration::from_secs(10);

async fn connect(script: Script) -> (AcpTransport, Events, std::sync::Arc<fake_agent::Observed>) {
    connect_with_mode(script, AcpSessionMode::New).await
}

async fn connect_with_mode(
    script: Script,
    mode: AcpSessionMode,
) -> (AcpTransport, Events, std::sync::Arc<fake_agent::Observed>) {
    let (client_end, observed, _agent) = fake_agent::spawn(script);
    let mut transport = AcpTransport::new_with_args(PathBuf::from("fake-agent"), vec![], None)
        .expect("construct")
        .session_mode(mode);
    transport
        .connect_over(client_end)
        .await
        .expect("handshake against the fake agent");
    let events = transport.read_events().expect("event channel");
    (transport, events, observed)
}

async fn connect_local(
    script: Script,
    cwd: PathBuf,
) -> (AcpTransport, Events, std::sync::Arc<fake_agent::Observed>) {
    let (client_end, observed, _agent) = fake_agent::spawn(script);
    let mut transport = AcpTransport::new_with_args(PathBuf::from("fake-agent"), vec![], Some(cwd))
        .expect("construct")
        .local_effects();
    transport.connect_over(client_end).await.expect("handshake");
    let events = transport.read_events().expect("events");
    (transport, events, observed)
}

async fn connect_allowed_local(
    script: Script,
    cwd: PathBuf,
) -> (AcpTransport, Events, std::sync::Arc<fake_agent::Observed>) {
    let (mut transport, events, observed) = connect_local(script, cwd).await;
    let mut permissions = transport.take_permission_rx().expect("permissions");
    tokio::spawn(async move {
        while let Some(request) = permissions.recv().await {
            let _ = request
                .response_tx
                .send(AcpPermissionResponse::Selected("allow".into()));
        }
    });
    (transport, events, observed)
}

async fn next(events: &mut Events) -> magistr_acp::Result<AcpEvent> {
    tokio::time::timeout(WAIT, events.recv())
        .await
        .expect("an event before the timeout")
        .expect("channel open")
}

/// Drain until an event matching `pred` arrives, returning it.
async fn next_matching(events: &mut Events, pred: impl Fn(&AcpEvent) -> bool) -> AcpEvent {
    loop {
        match next(events).await {
            Ok(ev) if pred(&ev) => return ev,
            Ok(_) => {}
            Err(e) => panic!("transport error while waiting: {e}"),
        }
    }
}

#[tokio::test]
async fn handshake_emits_initialized_with_the_agents_config_options() {
    let (transport, mut events, _) = connect(Script::default()).await;
    assert!(transport.is_ready());
    assert_eq!(
        transport.child_pgid(),
        0,
        "no subprocess for an in-process agent"
    );

    match next(&mut events).await.expect("initialized") {
        AcpEvent::Initialized {
            session_id,
            config_options,
            agent_capabilities,
            agent_meta,
        } => {
            assert!(agent_meta.is_none(), "the plain fake advertises nothing");
            assert_eq!(session_id, SESSION_ID);
            assert!(!agent_capabilities.load_session);
            let opts = config_options.expect("config options");
            assert_eq!(opts[0].id.to_string(), "model");
            let json = serde_json::to_string(&opts).expect("serialize");
            assert!(json.contains(FABLE), "picker must list Fable: {json}");
        }
        other => panic!("expected Initialized, got {other:?}"),
    }
}

fn strict_meta() -> Meta {
    let mut meta = Meta::new();
    meta.insert(
        "claudeCode".to_owned(),
        serde_json::json!({"options": {"strictMcpConfig": true}}),
    );
    meta
}

#[tokio::test]
async fn the_session_meta_rides_on_session_new() {
    let (client_end, observed, _agent) = fake_agent::spawn(Script::default());
    let mut transport = AcpTransport::new_with_args(PathBuf::from("fake-agent"), vec![], None)
        .expect("construct")
        .session_meta(Some(strict_meta()));
    transport.connect_over(client_end).await.expect("handshake");
    assert_eq!(
        *observed.session_meta.lock().expect("lock"),
        Some(strict_meta())
    );
}

#[tokio::test]
async fn the_session_meta_rides_on_session_load_too() {
    let (client_end, observed, _agent) = fake_agent::spawn(Script {
        supports_load: true,
        ..Script::default()
    });
    let mut transport = AcpTransport::new_with_args(PathBuf::from("fake-agent"), vec![], None)
        .expect("construct")
        .session_mode(AcpSessionMode::Load {
            session_id: SESSION_ID.to_string(),
        })
        .session_meta(Some(strict_meta()));
    transport.connect_over(client_end).await.expect("handshake");
    assert_eq!(
        *observed.session_meta.lock().expect("lock"),
        Some(strict_meta())
    );
    assert_eq!(
        *observed.loaded_session.lock().expect("lock"),
        Some(SESSION_ID.to_string())
    );
}

#[tokio::test]
async fn a_prompt_streams_updates_and_ends_with_the_prompt_response() {
    let (transport, mut events, _) = connect(Script::default()).await;
    let _ = next(&mut events).await; // Initialized
    transport.prompt("hi").expect("prompt");

    let chunk = next_matching(&mut events, |e| matches!(e, AcpEvent::SessionUpdate { .. })).await;
    let AcpEvent::SessionUpdate { update, session_id } = chunk else {
        unreachable!()
    };
    assert_eq!(session_id, SESSION_ID);
    assert!(matches!(*update, SessionUpdate::AgentMessageChunk(_)));

    let done = next_matching(&mut events, |e| {
        matches!(e, AcpEvent::PromptResponse { .. })
    })
    .await;
    let AcpEvent::PromptResponse { response, .. } = done else {
        unreachable!()
    };
    assert_eq!(response.stop_reason, StopReason::EndTurn);
}

#[tokio::test]
async fn a_permission_request_round_trips_through_the_application() {
    let (mut transport, mut events, observed) = connect(Script {
        ask_permission: true,
        ..Script::default()
    })
    .await;
    let mut perm_rx = transport.take_permission_rx().expect("permission channel");
    let _ = next(&mut events).await;
    transport.prompt("do it").expect("prompt");

    let request = tokio::time::timeout(WAIT, perm_rx.recv())
        .await
        .expect("permission request in time")
        .expect("channel open");
    assert_eq!(request.tool_title, "Bash");
    assert_eq!(request.options.len(), 2);
    request
        .response_tx
        .send(AcpPermissionResponse::Selected("reject".into()))
        .expect("answer");

    let _ = next_matching(&mut events, |e| {
        matches!(e, AcpEvent::PromptResponse { .. })
    })
    .await;
    let outcome = observed.permission_outcome.lock().expect("lock").take();
    match outcome {
        Some(RequestPermissionOutcome::Selected(s)) => {
            assert_eq!(s.option_id.to_string(), "reject")
        }
        other => panic!("agent saw {other:?}"),
    }
    // The UI was told about it via the event stream as well.
    // (permission_request System event precedes the PromptResponse.)
}

#[tokio::test]
async fn a_dropped_permission_receiver_denies_instead_of_wedging() {
    let (mut transport, mut events, observed) = connect(Script {
        ask_permission: true,
        ..Script::default()
    })
    .await;
    // The application declines to handle permissions at all: the transport
    // answers on its behalf — with the reject, never an allow nobody gave
    //  — so the turn still completes.
    drop(transport.take_permission_rx());
    let _ = next(&mut events).await;
    transport.prompt("do it").expect("prompt");
    let _ = next_matching(&mut events, |e| {
        matches!(e, AcpEvent::PromptResponse { .. })
    })
    .await;
    match observed.permission_outcome.lock().expect("lock").take() {
        Some(RequestPermissionOutcome::Selected(s)) => {
            assert_eq!(s.option_id.to_string(), "reject")
        }
        other => panic!("agent saw {other:?}"),
    }
}

#[tokio::test]
async fn the_agent_can_read_files_through_the_client() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("read-me.txt");
    std::fs::write(&path, "line one\nline two").expect("write fixture");
    let (transport, mut events, observed) = connect_allowed_local(
        Script {
            read_file: Some(path),
            ..Script::default()
        },
        dir.path().to_path_buf(),
    )
    .await;
    let _ = next(&mut events).await;
    transport.prompt("read").expect("prompt");
    let _ = next_matching(&mut events, |e| {
        matches!(e, AcpEvent::PromptResponse { .. })
    })
    .await;
    assert_eq!(
        observed.file_content.lock().expect("lock").as_deref(),
        Some("line one\nline two")
    );
}

#[tokio::test]
async fn the_agent_can_run_a_terminal_through_the_client() {
    let (transport, mut events, observed) = connect_allowed_local(
        Script {
            run_terminal: Some(("sh".into(), vec!["-c".into(), "echo from-terminal".into()])),
            ..Script::default()
        },
        std::env::current_dir().expect("cwd"),
    )
    .await;
    let _ = next(&mut events).await;
    transport.prompt("run").expect("prompt");

    let mut subtypes = Vec::new();
    loop {
        match next(&mut events).await.expect("event") {
            AcpEvent::System { subtype, .. } => subtypes.push(subtype),
            AcpEvent::PromptResponse { .. } => break,
            _ => {}
        }
    }
    assert_eq!(
        subtypes,
        vec!["permission_request", "terminal_create", "terminal_release"]
    );
    let exit = observed
        .terminal_exit
        .lock()
        .expect("lock")
        .take()
        .expect("exit");
    assert_eq!(exit.exit_code, Some(0));
    // Output may or may not be flushed by the time the agent asked; it must
    // at least not be an error.
    assert!(observed.terminal_output.lock().expect("lock").is_some());
}

/// The agent's command prints colours: the raw bytes stream to the
/// consumer in order while it runs, and the agent reads plain text.
#[cfg(unix)]
#[tokio::test]
async fn a_terminals_bytes_stream_raw_and_the_agent_reads_stripped_text() {
    let (transport, mut events, observed) = connect_allowed_local(
        Script {
            run_terminal: Some((
                "sh".into(),
                vec![
                    "-c".into(),
                    "test -t 1 && printf '\\033[31mred\\033[0m\\n'; printf 'done\\n'".into(),
                ],
            )),
            ..Script::default()
        },
        std::env::current_dir().expect("cwd"),
    )
    .await;
    let _ = next(&mut events).await;
    transport.prompt("run").expect("prompt");

    let mut created = None;
    let mut streamed: Vec<(String, Vec<u8>)> = Vec::new();
    loop {
        match next(&mut events).await.expect("event") {
            AcpEvent::System { subtype, data } if subtype == "terminal_create" => {
                created = Some(data);
            }
            AcpEvent::Terminal { terminal_id, bytes } => streamed.push((terminal_id, bytes)),
            AcpEvent::PromptResponse { .. } => break,
            _ => {}
        }
    }
    let created = created.expect("terminal_create");
    assert_eq!(created["pty"], true);
    assert_eq!(created["pane"], "live");
    let tid = created["terminal_id"].as_str().expect("id");
    assert!(streamed.iter().all(|(id, _)| id == tid));
    let bytes: Vec<u8> = streamed.into_iter().flat_map(|(_, b)| b).collect();
    assert_eq!(bytes, b"\x1b[31mred\x1b[0m\r\ndone\r\n");

    let exit = observed
        .terminal_exit
        .lock()
        .expect("lock")
        .take()
        .expect("exit");
    assert_eq!(exit.exit_code, Some(0), "stdout was a tty");
    assert_eq!(
        observed.terminal_output.lock().expect("lock").as_deref(),
        Some("red\ndone\n")
    );
}

#[tokio::test]
async fn cancel_mid_turn_ends_the_turn_with_cancelled() {
    let (transport, mut events, observed) = connect(Script {
        hang_until_cancel: true,
        ..Script::default()
    })
    .await;
    let _ = next(&mut events).await;
    transport.prompt("think forever").expect("prompt");
    // Wait until the turn is provably in flight.
    let _ = next_matching(&mut events, |e| matches!(e, AcpEvent::SessionUpdate { .. })).await;

    transport.cancel().expect("cancel");
    let done = next_matching(&mut events, |e| {
        matches!(e, AcpEvent::PromptResponse { .. })
    })
    .await;
    let AcpEvent::PromptResponse { response, .. } = done else {
        unreachable!()
    };
    assert_eq!(response.stop_reason, StopReason::Cancelled);
    assert_eq!(
        observed.cancels.load(std::sync::atomic::Ordering::SeqCst),
        1
    );
}

#[tokio::test]
async fn set_config_option_returns_the_agents_updated_options() {
    let (transport, mut events, observed) = connect(Script::default()).await;
    let _ = next(&mut events).await;
    let updated = transport
        .set_config_option(
            SESSION_ID.into(),
            "model".into(),
            SessionConfigOptionValue::ValueId {
                value: FABLE.into(),
            },
        )
        .await
        .expect("set config option");
    let json = serde_json::to_string(&updated).expect("serialize");
    assert!(
        json.contains(&format!("\"currentValue\":\"{FABLE}\"")),
        "got {json}"
    );
    assert_eq!(
        observed.config_sets.lock().expect("lock").as_slice(),
        &[("model".to_string(), FABLE.to_string())]
    );
}

#[tokio::test]
async fn list_sessions_round_trips() {
    let (transport, mut events, _) = connect(Script::default()).await;
    let _ = next(&mut events).await;
    let sessions = transport.list_sessions(None).await.expect("list");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id.to_string(), SESSION_ID);
}

#[tokio::test]
async fn load_mode_resumes_the_requested_session_when_supported() {
    let (_transport, mut events, observed) = connect_with_mode(
        Script {
            supports_load: true,
            ..Script::default()
        },
        AcpSessionMode::Load {
            session_id: "old-1".into(),
        },
    )
    .await;
    match next(&mut events).await.expect("initialized") {
        AcpEvent::Initialized { session_id, .. } => assert_eq!(session_id, "old-1"),
        other => panic!("expected Initialized, got {other:?}"),
    }
    assert_eq!(
        observed.loaded_session.lock().expect("lock").as_deref(),
        Some("old-1")
    );
}

#[tokio::test]
async fn load_mode_rejects_unsupported_resume_without_creating_an_empty_session() {
    let (client_end, observed, _agent) = fake_agent::spawn(Script::default());
    let mut transport = AcpTransport::new_with_args(PathBuf::from("fake-agent"), vec![], None)
        .expect("construct")
        .session_mode(AcpSessionMode::Load {
            session_id: "old-1".into(),
        });
    let error = transport
        .connect_over(client_end)
        .await
        .expect_err("resume must be supported");
    assert!(error.to_string().contains("old-1"), "{error}");
    assert!(error.to_string().contains("does not support"), "{error}");
    assert!(!transport.is_ready());
    assert!(transport.prompt("must not infer").is_err());
    assert!(observed.loaded_session.lock().expect("lock").is_none());
    assert!(observed.session_meta.lock().expect("lock").is_none());
}

#[tokio::test]
async fn an_agent_that_goes_away_is_reported_as_a_transport_error() {
    let (client_end, _observed, agent) = fake_agent::spawn(Script::default());
    let mut transport =
        AcpTransport::new_with_args(PathBuf::from("fake-agent"), vec![], None).expect("construct");
    transport.connect_over(client_end).await.expect("handshake");
    let mut events = transport.read_events().expect("events");
    let _ = next(&mut events).await;

    agent.abort();
    let err = tokio::time::timeout(WAIT, async {
        loop {
            if let Some(Err(e)) = events.recv().await {
                return e;
            }
        }
    })
    .await
    .expect("error before the timeout");
    assert!(
        matches!(err, AcpError::Transport(ref m) if m.contains("closed")),
        "got {err}"
    );
}

#[tokio::test]
async fn close_ends_the_connection_and_the_agent() {
    let (client_end, _observed, agent) = fake_agent::spawn(Script::default());
    let mut transport =
        AcpTransport::new_with_args(PathBuf::from("fake-agent"), vec![], None).expect("construct");
    transport.connect_over(client_end).await.expect("handshake");
    transport.close().await.expect("close");
    assert!(!transport.is_ready());
    assert!(transport.prompt("late").is_err());
    let finished = tokio::time::timeout(WAIT, agent).await;
    assert!(
        finished.is_ok(),
        "agent task should finish once the client hangs up"
    );
}

#[tokio::test]
async fn initialized_carries_the_initialize_meta_the_agent_advertised() {
    let (_transport, mut events, _) = connect(Script {
        goal_extension: true,
        ..Script::default()
    })
    .await;
    match next(&mut events).await.expect("initialized") {
        AcpEvent::Initialized { agent_meta, .. } => {
            assert_eq!(agent_meta, Some(fake_agent::goal_meta()));
        }
        other => panic!("expected Initialized, got {other:?}"),
    }
}

#[tokio::test]
async fn an_extension_request_is_answered_off_the_loop_and_its_notification_streams() {
    let (transport, mut events, observed) = connect(Script {
        goal_extension: true,
        ..Script::default()
    })
    .await;
    let _ = next(&mut events).await;
    let answer = transport
        .extension(
            "_session/goal",
            serde_json::json!({"sessionId": SESSION_ID, "action": "set", "objective": "be done"}),
        )
        .expect("issued");
    // The echo arrives on the event stream while the request is out.
    let echo = next_matching(&mut events, |e| {
        matches!(e, AcpEvent::SessionUpdate { update, .. } if matches!(**update, SessionUpdate::SessionInfoUpdate(_)))
    })
    .await;
    let AcpEvent::SessionUpdate { update, .. } = echo else {
        unreachable!()
    };
    let SessionUpdate::SessionInfoUpdate(info) = *update else {
        unreachable!()
    };
    assert_eq!(
        info.meta.expect("meta")["goal"]["objective"],
        serde_json::json!("be done")
    );
    let result = tokio::time::timeout(WAIT, answer)
        .await
        .expect("answered in time")
        .expect("the reply was sent")
        .expect("the agent answered");
    assert_eq!(result, serde_json::json!({}));
    assert_eq!(
        observed.goals.lock().expect("lock").as_slice(),
        &[("set".to_string(), Some("be done".to_string()))]
    );
}

#[tokio::test]
async fn an_extension_request_mid_turn_is_served_too() {
    let (transport, mut events, observed) = connect(Script {
        goal_extension: true,
        hang_until_cancel: true,
        ..Script::default()
    })
    .await;
    let _ = next(&mut events).await;
    transport.prompt("work").expect("prompt");
    let _ = next_matching(&mut events, |e| matches!(e, AcpEvent::SessionUpdate { .. })).await;
    let answer = transport
        .extension(
            "_session/goal",
            serde_json::json!({"sessionId": SESSION_ID, "action": "clear"}),
        )
        .expect("issued mid-turn");
    let result = tokio::time::timeout(WAIT, answer)
        .await
        .expect("answered while the turn is still in flight")
        .expect("reply sent");
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(
        observed.goals.lock().expect("lock").as_slice(),
        &[("clear".to_string(), None)]
    );
    transport.cancel().expect("cancel");
    let _ = next_matching(&mut events, |e| {
        matches!(e, AcpEvent::PromptResponse { .. })
    })
    .await;
}

#[tokio::test]
async fn an_unknown_extension_method_fails_on_the_receiver_not_the_call() {
    let (transport, mut events, _) = connect(Script::default()).await;
    let _ = next(&mut events).await;
    let answer = transport
        .extension("_nobody/home", serde_json::json!({}))
        .expect("issuing never fails on a live session");
    let result = tokio::time::timeout(WAIT, answer)
        .await
        .expect("answered")
        .expect("reply sent");
    assert!(
        matches!(result, Err(AcpError::Transport(ref m)) if m.contains("_nobody/home")),
        "got {result:?}"
    );
}

#[tokio::test]
async fn a_sender_outlives_the_transport_handle_and_a_tap_sees_every_ok_event() {
    let (client_end, _observed, _agent) = fake_agent::spawn(Script::default());
    let mut transport =
        AcpTransport::new_with_args(PathBuf::from("fake-agent"), vec![], None).expect("construct");
    transport.connect_over(client_end).await.expect("handshake");
    let sender = transport.sender().expect("a sender after connect");
    let mut tap = transport.tap_events().expect("a tap before read_events");
    let mut events = transport.read_events().expect("the stream, still whole");
    assert!(transport.tap_events().is_none(), "nothing left to tap");
    // The session owner takes the transport away; the sender still works.
    let owner = tokio::spawn(async move {
        let mut seen = Vec::new();
        while let Some(Ok(event)) = events.recv().await {
            let done = matches!(event, AcpEvent::PromptResponse { .. });
            seen.push(event);
            if done {
                break;
            }
        }
        (transport, seen)
    });
    sender
        .prompt("via the sender")
        .expect("prompt through the sender");
    let (_transport, seen) = tokio::time::timeout(WAIT, owner)
        .await
        .expect("the owner saw the turn end")
        .expect("no panic");
    let mut tapped = Vec::new();
    while let Ok(Some(event)) = tokio::time::timeout(Duration::from_millis(200), tap.recv()).await {
        tapped.push(event);
    }
    let kinds = |events: &[AcpEvent]| -> Vec<String> {
        events
            .iter()
            .map(|e| match e {
                AcpEvent::Initialized { .. } => "initialized".to_owned(),
                AcpEvent::SessionUpdate { .. } => "update".to_owned(),
                AcpEvent::PromptResponse { .. } => "prompt_response".to_owned(),
                AcpEvent::Terminal { .. } => "terminal".to_owned(),
                AcpEvent::System { subtype, .. } => subtype.clone(),
            })
            .collect()
    };
    assert_eq!(kinds(&tapped), kinds(&seen));
    assert_eq!(
        kinds(&seen),
        ["initialized", "update", "prompt_response"],
        "the owner's stream is the whole stream"
    );
}

#[tokio::test]
async fn bare_transport_advertises_no_host_effects_and_refuses_a_direct_file_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("private.txt");
    std::fs::write(&path, "not available").expect("fixture");
    let (transport, mut events, observed) = connect(Script {
        read_file: Some(path.clone()),
        write_file: Some((dir.path().join("should-not-exist.txt"), "forbidden".into())),
        run_terminal: Some(("sh".into(), vec!["-c".into(), "exit 0".into()])),
        ..Script::default()
    })
    .await;
    let caps = observed
        .client_capabilities
        .lock()
        .expect("lock")
        .clone()
        .expect("caps");
    assert!(!caps.fs.read_text_file);
    assert!(!caps.fs.write_text_file);
    assert!(!caps.terminal);
    transport.prompt("read").expect("prompt");
    let _ = next_matching(&mut events, |event| {
        matches!(event, AcpEvent::PromptResponse { .. })
    })
    .await;
    assert!(observed.file_content.lock().expect("lock").is_none());
    let errors = observed.effect_errors.lock().expect("lock");
    assert_eq!(errors.len(), 3);
    assert!(errors.iter().all(|error| i32::from(error.code) == -32601));
    assert!(!dir.path().join("should-not-exist.txt").exists());
    assert!(observed.terminal_exit.lock().expect("lock").is_none());
}

#[tokio::test]
async fn local_effects_are_advertised_but_each_read_still_needs_host_authorization() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("private.txt");
    std::fs::write(&path, "private content").expect("fixture");
    let (mut transport, mut events, observed) = connect_local(
        Script {
            read_file: Some(path.clone()),
            ..Script::default()
        },
        dir.path().to_path_buf(),
    )
    .await;
    let caps = observed
        .client_capabilities
        .lock()
        .expect("lock")
        .clone()
        .expect("caps");
    assert!(caps.fs.read_text_file && caps.fs.write_text_file && caps.terminal);
    let mut permissions = transport.take_permission_rx().expect("permissions");
    transport.prompt("read").expect("prompt");
    let request = tokio::time::timeout(WAIT, permissions.recv())
        .await
        .expect("request")
        .expect("open");
    assert_eq!(request.tool_title, "Read");
    assert_eq!(
        request.tool_input["file_path"],
        std::fs::canonicalize(path)
            .expect("canonical")
            .to_str()
            .expect("UTF-8")
    );
    request
        .response_tx
        .send(AcpPermissionResponse::Selected("reject".into()))
        .expect("deny");
    let _ = next_matching(&mut events, |event| {
        matches!(event, AcpEvent::PromptResponse { .. })
    })
    .await;
    assert!(observed.file_content.lock().expect("lock").is_none());
    let errors = observed.effect_errors.lock().expect("lock");
    assert_eq!(i32::from(errors[0].code), -32003);
    assert_eq!(
        errors[0].data.as_ref().expect("category")["effect"],
        "denied"
    );
}

#[tokio::test]
async fn a_pending_effect_permission_keeps_dispatch_live_and_cancel_refuses_a_late_allow() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("pending.txt");
    std::fs::write(&path, "must not read").expect("fixture");
    let (mut transport, mut events, observed) = connect_local(
        Script {
            read_file: Some(path),
            goal_extension: true,
            ..Script::default()
        },
        dir.path().to_path_buf(),
    )
    .await;
    let mut permissions = transport.take_permission_rx().expect("permissions");
    transport.prompt("read").expect("prompt");
    let request = tokio::time::timeout(WAIT, permissions.recv())
        .await
        .expect("request")
        .expect("open");
    let response = transport.extension("_session/goal", serde_json::json!({"sessionId": SESSION_ID, "action": "set", "objective": "still responsive"})).expect("extension");
    tokio::time::timeout(WAIT, response)
        .await
        .expect("dispatch stays live")
        .expect("reply")
        .expect("response");
    let _ = next_matching(&mut events, |event| matches!(event, AcpEvent::SessionUpdate { update, .. } if matches!(**update, SessionUpdate::SessionInfoUpdate(_)))).await;
    transport.cancel().expect("cancel");
    let _ = next_matching(&mut events, |event| {
        matches!(event, AcpEvent::PromptResponse { .. })
    })
    .await;
    assert!(
        request
            .response_tx
            .send(AcpPermissionResponse::Selected("allow".into()))
            .is_err(),
        "cancel drops the stale reply"
    );
    assert!(observed.file_content.lock().expect("lock").is_none());
    let errors = observed.effect_errors.lock().expect("lock");
    assert_eq!(i32::from(errors[0].code), -32800);
    assert_eq!(
        observed.cancels.load(std::sync::atomic::Ordering::SeqCst),
        1
    );
}

#[tokio::test]
async fn a_missing_host_permission_consumer_denies_a_direct_effect() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("private.txt");
    std::fs::write(&path, "private").expect("fixture");
    let (mut transport, mut events, observed) = connect_local(
        Script {
            read_file: Some(path),
            ..Script::default()
        },
        dir.path().to_path_buf(),
    )
    .await;
    drop(transport.take_permission_rx());
    transport.prompt("read").expect("prompt");
    let _ = next_matching(&mut events, |event| {
        matches!(event, AcpEvent::PromptResponse { .. })
    })
    .await;
    assert!(observed.file_content.lock().expect("lock").is_none());
    assert_eq!(
        i32::from(observed.effect_errors.lock().expect("lock")[0].code),
        -32003
    );
}

#[tokio::test]
async fn a_denied_direct_write_cannot_change_the_filesystem() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("existing.txt");
    std::fs::write(&path, "original").expect("fixture");
    let (mut transport, mut events, observed) = connect_local(
        Script {
            write_file: Some((path.clone(), "replacement".into())),
            ..Script::default()
        },
        dir.path().to_path_buf(),
    )
    .await;
    let mut permissions = transport.take_permission_rx().expect("permissions");
    transport.prompt("write").expect("prompt");
    let request = tokio::time::timeout(WAIT, permissions.recv())
        .await
        .expect("request")
        .expect("open");
    assert_eq!(request.tool_title, "Write");
    request
        .response_tx
        .send(AcpPermissionResponse::Selected("reject".into()))
        .expect("deny");
    let _ = next_matching(&mut events, |event| {
        matches!(event, AcpEvent::PromptResponse { .. })
    })
    .await;
    assert_eq!(
        std::fs::read_to_string(path).expect("unchanged"),
        "original"
    );
    assert_eq!(
        i32::from(observed.effect_errors.lock().expect("lock")[0].code),
        -32003
    );
}

#[tokio::test]
async fn a_denied_direct_terminal_does_not_start_a_child() {
    let dir = tempfile::tempdir().expect("tempdir");
    let marker = dir.path().join("not-started");
    let (mut transport, mut events, observed) = connect_local(
        Script {
            run_terminal: Some((
                "sh".into(),
                vec!["-c".into(), format!("touch '{}'", marker.display())],
            )),
            ..Script::default()
        },
        dir.path().to_path_buf(),
    )
    .await;
    let mut permissions = transport.take_permission_rx().expect("permissions");
    transport.prompt("run").expect("prompt");
    let request = tokio::time::timeout(WAIT, permissions.recv())
        .await
        .expect("request")
        .expect("open");
    assert_eq!(request.tool_title, "Bash");
    assert_eq!(request.tool_input["args"][0], "-c");
    request
        .response_tx
        .send(AcpPermissionResponse::Selected("reject".into()))
        .expect("deny");
    let _ = next_matching(&mut events, |event| {
        matches!(event, AcpEvent::PromptResponse { .. })
    })
    .await;
    assert!(!marker.exists());
    assert_eq!(
        i32::from(observed.effect_errors.lock().expect("lock")[0].code),
        -32003
    );
}
