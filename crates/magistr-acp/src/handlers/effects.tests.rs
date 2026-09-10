use super::*;
use magistr_application::effects::EffectCapabilities;
use std::sync::atomic::AtomicUsize;

#[derive(Default)]
struct Port {
    executions: AtomicUsize,
}

impl EffectPort for Port {
    fn capabilities(&self) -> EffectCapabilities {
        EffectCapabilities {
            read_text_file: true,
            write_text_file: false,
            terminal: false,
        }
    }

    fn execute(
        &self,
        _: EffectRequest,
        _: Arc<dyn EffectAuthorizer>,
    ) -> EffectFuture<'_, EffectResponse> {
        Box::pin(async move {
            self.executions.fetch_add(1, Ordering::SeqCst);
            Ok(EffectResponse::Text("host content".into()))
        })
    }

    fn kill_terminal(&self, _: &str) -> std::result::Result<(), EffectError> {
        Ok(())
    }
    fn close(&self) {}
}

fn bridge(port: Arc<Port>) -> EffectBridge {
    let (permissions, _) = mpsc::unbounded_channel();
    let (events, _) = mpsc::unbounded_channel();
    let bridge = EffectBridge::new(Some(port), permissions, events);
    bridge.bind_session("session".into());
    bridge.begin_turn();
    bridge
}

#[tokio::test]
async fn queued_requests_capture_the_turn_before_the_task_is_scheduled() {
    let port = Arc::new(Port::default());
    let bridge = bridge(port.clone());
    let queued = bridge.for_request();
    bridge.cancel();
    bridge.begin_turn();
    let error = queued
        .read_text_file(ReadTextFileRequest::new("session", "/file"))
        .await
        .expect_err("old turn");
    assert_eq!(i32::from(error.code), -32800);
    assert_eq!(port.executions.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn another_session_cannot_execute_a_configured_capability() {
    let port = Arc::new(Port::default());
    let bridge = bridge(port.clone());
    let error = bridge
        .read_text_file(ReadTextFileRequest::new("foreign", "/file"))
        .await
        .expect_err("wrong session");
    assert_eq!(i32::from(error.code), -32602);
    assert_eq!(port.executions.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn independently_disabled_capabilities_are_not_dispatched() {
    let port = Arc::new(Port::default());
    let bridge = bridge(port.clone());
    let caps = bridge.client_capabilities();
    assert!(caps.fs.read_text_file);
    assert!(!caps.fs.write_text_file);
    assert!(!caps.terminal);
    let error = bridge
        .write_text_file(WriteTextFileRequest::new("session", "/file", "content"))
        .await
        .expect_err("write disabled");
    assert_eq!(i32::from(error.code), -32601);
    assert_eq!(port.executions.load(Ordering::SeqCst), 0);
}

#[cfg(unix)]
#[tokio::test]
async fn non_utf8_paths_cannot_be_retargeted_by_lossy_conversion() {
    use std::os::unix::ffi::OsStringExt;
    let port = Arc::new(Port::default());
    let bridge = bridge(port.clone());
    let path = std::path::PathBuf::from(std::ffi::OsString::from_vec(b"/file-\xff".to_vec()));
    let error = bridge
        .read_text_file(ReadTextFileRequest::new("session", path))
        .await
        .expect_err("invalid UTF-8");
    assert_eq!(i32::from(error.code), -32602);
    assert_eq!(port.executions.load(Ordering::SeqCst), 0);
}
