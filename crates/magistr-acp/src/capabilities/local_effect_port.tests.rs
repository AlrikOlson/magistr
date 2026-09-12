use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

struct Authorizer {
    requests: Mutex<Vec<EffectRequest>>,
    deny: bool,
    cancelled: AtomicBool,
    #[cfg(unix)]
    swap: Option<(PathBuf, PathBuf)>,
}

impl Authorizer {
    fn allow() -> Arc<Self> {
        Arc::new(Self {
            requests: Mutex::new(Vec::new()),
            deny: false,
            cancelled: AtomicBool::new(false),
            #[cfg(unix)]
            swap: None,
        })
    }
}

impl EffectAuthorizer for Authorizer {
    fn authorize(&self, request: EffectRequest) -> EffectFuture<'_, ()> {
        Box::pin(async move {
            self.requests.lock().unwrap().push(request);
            #[cfg(unix)]
            if let Some((link, target)) = &self.swap {
                std::fs::remove_file(link).unwrap();
                std::os::unix::fs::symlink(target, link).unwrap();
            }
            if self.deny {
                Err(EffectError::Denied("test policy refusal".into()))
            } else {
                Ok(())
            }
        })
    }
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

fn local(root: &Path) -> LocalEffectPort {
    let (events, _) = mpsc::unbounded_channel();
    LocalEffectPort::new(root.to_path_buf(), events)
}

fn write(path: &Path) -> EffectRequest {
    EffectRequest::WriteTextFile {
        session_id: "s".into(),
        path: path.display().to_string(),
        content: "new".into(),
    }
}

#[tokio::test]
async fn denied_write_never_creates_its_parent_directories() {
    let dir = tempfile::tempdir().unwrap();
    let port = local(dir.path());
    let auth = Arc::new(Authorizer {
        deny: true,
        ..Arc::try_unwrap(Authorizer::allow()).ok().unwrap()
    });
    let result = port
        .execute(write(&dir.path().join("missing/child.txt")), auth)
        .await;
    assert!(matches!(result, Err(EffectError::Denied(_))));
    assert!(!dir.path().join("missing").exists());
}

#[tokio::test]
async fn allowed_write_and_read_reach_the_same_resolved_file() {
    let dir = tempfile::tempdir().unwrap();
    let port = local(dir.path());
    let auth = Authorizer::allow();
    let path = dir.path().join("nested/file.txt");
    assert_eq!(
        port.execute(write(&path), auth.clone()).await.unwrap(),
        EffectResponse::Written
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
    let result = port
        .execute(
            EffectRequest::ReadTextFile {
                session_id: "s".into(),
                path: path.display().to_string(),
                line: None,
                limit: None,
            },
            auth.clone(),
        )
        .await
        .unwrap();
    assert_eq!(result, EffectResponse::Text("new".into()));
    assert_eq!(auth.requests.lock().unwrap().len(), 2);
}

#[cfg(unix)]
#[tokio::test]
async fn authorization_observes_existing_symlink_ancestors_of_new_files() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("real")).unwrap();
    std::os::unix::fs::symlink(dir.path().join("real"), dir.path().join("alias")).unwrap();
    let auth = Authorizer::allow();
    local(dir.path())
        .execute(write(&dir.path().join("alias/new.txt")), auth.clone())
        .await
        .unwrap();
    let expected = std::fs::canonicalize(dir.path().join("real/new.txt")).unwrap();
    assert!(
        matches!(&auth.requests.lock().unwrap()[0], EffectRequest::WriteTextFile { path, .. } if Path::new(path) == expected)
    );
}

#[cfg(unix)]
#[tokio::test]
async fn symlink_retarget_during_authorization_refuses_the_write() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first");
    let second = dir.path().join("second");
    std::fs::create_dir(&first).unwrap();
    std::fs::create_dir(&second).unwrap();
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&first, &link).unwrap();
    let auth = Arc::new(Authorizer {
        swap: Some((link.clone(), second.clone())),
        ..Arc::try_unwrap(Authorizer::allow()).ok().unwrap()
    });
    let result = local(dir.path())
        .execute(write(&link.join("new.txt")), auth)
        .await;
    assert!(matches!(result, Err(EffectError::Denied(reason)) if reason.contains("changed")));
    assert!(!first.join("new.txt").exists());
    assert!(!second.join("new.txt").exists());
}

#[tokio::test]
async fn cancelled_admission_never_requests_authorization_or_writes() {
    let dir = tempfile::tempdir().unwrap();
    let auth = Authorizer::allow();
    auth.cancelled.store(true, Ordering::SeqCst);
    let path = dir.path().join("new.txt");
    assert_eq!(
        local(dir.path()).execute(write(&path), auth.clone()).await,
        Err(EffectError::Cancelled)
    );
    assert!(auth.requests.lock().unwrap().is_empty());
    assert!(!path.exists());
}

#[tokio::test]
async fn closing_a_port_prevents_later_admission_even_with_an_allowing_policy() {
    let dir = tempfile::tempdir().unwrap();
    let port = local(dir.path());
    port.close();
    let auth = Authorizer::allow();
    assert_eq!(
        port.execute(write(&dir.path().join("new.txt")), auth.clone())
            .await,
        Err(EffectError::Cancelled)
    );
    assert!(auth.requests.lock().unwrap().is_empty());
    assert!(!dir.path().join("new.txt").exists());
}

#[tokio::test]
async fn workspace_escape_is_denied_before_authorization() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let auth = Authorizer::allow();
    let result = local(root.path())
        .execute(write(&outside.path().join("new.txt")), auth.clone())
        .await;
    assert!(matches!(result, Err(EffectError::Denied(_))));
    assert!(auth.requests.lock().unwrap().is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn process_authorization_uses_resolved_executable_cwd_and_exact_arguments() {
    let dir = tempfile::tempdir().unwrap();
    let port = local(dir.path());
    let auth = Authorizer::allow();
    let request = EffectRequest::CreateTerminal {
        session_id: "s".into(),
        command: "sh".into(),
        executable: None,
        args: vec!["-c".into(), "printf '%s' \"$EFFECT_TEST_VALUE\"".into()],
        env: vec![("EFFECT_TEST_VALUE".into(), "effect-ok".into())],
        cwd: None,
        output_byte_limit: Some(1024),
    };
    let EffectResponse::TerminalCreated(id) = port.execute(request, auth.clone()).await.unwrap()
    else {
        panic!("terminal")
    };
    let captured = auth.requests.lock().unwrap()[0].clone();
    assert!(
        matches!(captured, EffectRequest::CreateTerminal { executable: Some(command), cwd: Some(cwd), env, .. }
        if Path::new(&command).is_absolute() && Path::new(&cwd) == std::fs::canonicalize(dir.path()).unwrap()
        && env == vec![("EFFECT_TEST_VALUE".into(), "effect-ok".into())])
    );
    let other = port
        .execute(
            EffectRequest::TerminalOutput {
                session_id: "other".into(),
                terminal_id: id.clone(),
            },
            auth.clone(),
        )
        .await;
    assert!(matches!(other, Err(EffectError::Denied(_))));
    let exit = port
        .execute(
            EffectRequest::WaitForTerminalExit {
                session_id: "s".into(),
                terminal_id: id.clone(),
            },
            auth.clone(),
        )
        .await
        .unwrap();
    assert!(matches!(
        exit,
        EffectResponse::TerminalExited(TerminalExit {
            exit_code: Some(0),
            ..
        })
    ));
    let output = port
        .execute(
            EffectRequest::TerminalOutput {
                session_id: "s".into(),
                terminal_id: id.clone(),
            },
            auth.clone(),
        )
        .await
        .unwrap();
    assert!(
        matches!(output, EffectResponse::TerminalOutput { output, .. } if output.contains("effect-ok"))
    );
    port.execute(
        EffectRequest::ReleaseTerminal {
            session_id: "s".into(),
            terminal_id: id.clone(),
        },
        auth.clone(),
    )
    .await
    .unwrap();
    assert!(matches!(
        port.execute(
            EffectRequest::TerminalOutput {
                session_id: "s".into(),
                terminal_id: id
            },
            auth
        )
        .await,
        Err(EffectError::InvalidRequest(_))
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn denied_process_does_not_start_a_child() {
    let dir = tempfile::tempdir().unwrap();
    let auth = Arc::new(Authorizer {
        deny: true,
        ..Arc::try_unwrap(Authorizer::allow()).ok().unwrap()
    });
    let result = local(dir.path())
        .execute(
            EffectRequest::CreateTerminal {
                session_id: "s".into(),
                command: "sh".into(),
                executable: None,
                args: vec!["-c".into(), "printf bad > marker".into()],
                env: vec![],
                cwd: None,
                output_byte_limit: None,
            },
            auth,
        )
        .await;
    assert!(matches!(result, Err(EffectError::Denied(_))));
    assert!(!dir.path().join("marker").exists());
}

#[cfg(unix)]
#[test]
fn resolver_follows_a_symlink_before_parent_components_and_detects_loops() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("real/deep")).unwrap();
    std::os::unix::fs::symlink(dir.path().join("real/deep"), dir.path().join("alias")).unwrap();
    assert_eq!(
        resolve_target(dir.path(), Path::new("alias/../new")).unwrap(),
        std::fs::canonicalize(dir.path().join("real"))
            .unwrap()
            .join("new")
    );
    std::os::unix::fs::symlink("loop", dir.path().join("loop")).unwrap();
    assert!(matches!(
        resolve_target(dir.path(), Path::new("loop/new")),
        Err(EffectError::InvalidRequest(_))
    ));
}

#[test]
fn resolver_accepts_a_canonical_root_and_restates_requests_under_it() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("real")).unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    assert_eq!(resolve_target(&root, &root).unwrap(), root);
    assert_eq!(
        resolve_target(&root, Path::new("real/new")).unwrap(),
        root.join("real").join("new")
    );
    // The same file spelled with the caller's original prefix form.
    let plain = dir.path().join("real").join("new");
    let resolved = resolve_target(&root, &plain).unwrap();
    assert_eq!(resolved, root.join("real").join("new"));
    assert!(resolved.starts_with(&root));
}

/// A source writer that records what reached it and answers a receipt.
struct RecordingWriter {
    writes: Mutex<Vec<(String, String)>>,
    refuse: Option<EffectError>,
}

impl RecordingWriter {
    fn accepting() -> Arc<Self> {
        Arc::new(Self {
            writes: Mutex::new(Vec::new()),
            refuse: None,
        })
    }
}

impl magistr_application::effects::SourceWriter for RecordingWriter {
    fn write_text(
        &self,
        path: &str,
        content: &str,
    ) -> EffectFuture<'_, magistr_application::effects::SourceWriteReceipt> {
        let path = path.to_owned();
        let content = content.to_owned();
        Box::pin(async move {
            if let Some(refusal) = &self.refuse {
                return Err(refusal.clone());
            }
            self.writes.lock().unwrap().push((path, content));
            Ok(magistr_application::effects::SourceWriteReceipt {
                journal_seq: Some(7),
                revision: Some("rev".into()),
                changed: true,
            })
        })
    }
}

fn routed(
    root: &Path,
    writer: Arc<RecordingWriter>,
) -> (LocalEffectPort, mpsc::UnboundedReceiver<Result<AcpEvent>>) {
    let (events, rx) = mpsc::unbounded_channel();
    (
        LocalEffectPort::new(root.to_path_buf(), events).with_source_writer(writer),
        rx,
    )
}

fn write_events(rx: &mut mpsc::UnboundedReceiver<Result<AcpEvent>>) -> Vec<serde_json::Value> {
    std::iter::from_fn(|| rx.try_recv().ok())
        .filter_map(|event| match event {
            Ok(AcpEvent::System { subtype, data }) if subtype == "fs_write" => Some(data),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn an_authorized_write_is_routed_through_the_source_writer_after_the_decision() {
    let dir = tempfile::tempdir().unwrap();
    let writer = RecordingWriter::accepting();
    let (port, mut rx) = routed(dir.path(), writer.clone());
    let auth = Authorizer::allow();
    let path = dir.path().join("src/new.txt");
    assert_eq!(
        port.execute(write(&path), auth.clone()).await.unwrap(),
        EffectResponse::Written
    );
    // The writer, not the port, owns the bytes: nothing was written directly.
    assert!(!path.exists());
    // Resolved the way the port resolves it: `canonicalize` would add the
    // verbatim prefix on Windows, which the port never produces.
    let expected = resolve_target(dir.path(), dir.path())
        .unwrap()
        .join("src")
        .join("new.txt");
    assert_eq!(
        writer.writes.lock().unwrap().as_slice(),
        &[(expected.display().to_string(), "new".to_owned())]
    );
    assert_eq!(auth.requests.lock().unwrap().len(), 1, "authorized first");
    let events = write_events(&mut rx);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["route"], "workspace");
    assert_eq!(events[0]["journal_seq"], 7);
    assert_eq!(events[0]["revision"], "rev");
    assert_eq!(events[0]["changed"], true);
}

#[tokio::test]
async fn a_denied_or_cancelled_write_never_reaches_the_source_writer() {
    let dir = tempfile::tempdir().unwrap();
    let writer = RecordingWriter::accepting();
    let (port, mut rx) = routed(dir.path(), writer.clone());
    let denied = Arc::new(Authorizer {
        deny: true,
        ..Arc::try_unwrap(Authorizer::allow()).ok().unwrap()
    });
    assert!(matches!(
        port.execute(write(&dir.path().join("a.txt")), denied).await,
        Err(EffectError::Denied(_))
    ));
    let cancelled = Authorizer::allow();
    cancelled.cancelled.store(true, Ordering::SeqCst);
    assert_eq!(
        port.execute(write(&dir.path().join("b.txt")), cancelled)
            .await,
        Err(EffectError::Cancelled)
    );
    assert!(writer.writes.lock().unwrap().is_empty());
    assert!(write_events(&mut rx).is_empty());
}

#[tokio::test]
async fn a_writer_refusal_is_the_effects_answer_and_nothing_lands_directly() {
    let dir = tempfile::tempdir().unwrap();
    let writer = Arc::new(RecordingWriter {
        writes: Mutex::new(Vec::new()),
        refuse: Some(EffectError::Failed("write a.txt: parse rejected".into())),
    });
    let (port, _rx) = routed(dir.path(), writer);
    let path = dir.path().join("a.txt");
    assert!(matches!(
        port.execute(write(&path), Authorizer::allow()).await,
        Err(EffectError::Failed(reason)) if reason.contains("parse rejected")
    ));
    assert!(!path.exists());
}

#[tokio::test]
async fn without_a_source_writer_the_write_is_direct_and_the_event_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let (events, mut rx) = mpsc::unbounded_channel();
    let port = LocalEffectPort::new(dir.path().to_path_buf(), events);
    let path = dir.path().join("direct.txt");
    port.execute(write(&path), Authorizer::allow())
        .await
        .unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
    let events = write_events(&mut rx);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["route"], "direct");
    assert!(events[0]["note"].as_str().unwrap().contains("unjournaled"));
}
