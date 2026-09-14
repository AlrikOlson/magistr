//! ACP terminal handlers backed by the shared owned-process runtime.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_client_protocol::Error;
use agent_client_protocol::schema::v1::{
    CreateTerminalRequest, CreateTerminalResponse, KillTerminalRequest, KillTerminalResponse,
    ReleaseTerminalRequest, ReleaseTerminalResponse, TerminalExitStatus, TerminalOutputRequest,
    TerminalOutputResponse, WaitForTerminalExitRequest, WaitForTerminalExitResponse,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::strip_escapes::EscapeStripper;
use crate::{AcpEvent, Result};

const INTERNAL_ERROR: i32 = -32603;
const INVALID_PARAMS: i32 = -32602;
const WAIT_POLL: Duration = Duration::from_millis(50);
const PTY_SIZE: (u16, u16) = (80, 24);

pub(crate) struct TerminalState {
    output: String,
    output_byte_limit: Option<u64>,
    truncated: bool,
    exit_status: Option<TerminalExitStatus>,
    done: bool,
    killed: bool,
    released: bool,
    cancel: CancellationToken,
}

pub(crate) type TerminalManager = Arc<Mutex<HashMap<String, TerminalState>>>;
type EventSender = mpsc::UnboundedSender<Result<AcpEvent>>;

pub(crate) fn new_terminal_manager() -> TerminalManager {
    Arc::new(Mutex::new(HashMap::new()))
}

fn lock(mgr: &TerminalManager) -> std::sync::MutexGuard<'_, HashMap<String, TerminalState>> {
    mgr.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn unknown(tid: &str) -> Error {
    Error::new(INVALID_PARAMS, format!("Unknown terminal ID: {tid}"))
}

fn next_terminal_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("term-{ts:x}-{:x}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

fn append_output(state: &mut TerminalState, text: &str) {
    let Some(limit) = state.output_byte_limit else {
        state.output.push_str(text);
        return;
    };
    let limit = usize::try_from(limit).unwrap_or(usize::MAX);
    if limit == 0 {
        state.truncated |= !text.is_empty();
        state.output.clear();
        return;
    }
    state.output.push_str(text);
    if state.output.len() > limit {
        let mut start = state.output.len() - limit;
        while !state.output.is_char_boundary(start) {
            start += 1;
        }
        state.output.drain(..start);
        state.truncated = true;
    }
}

fn ingest(
    mgr: &TerminalManager,
    tid: &str,
    event_tx: &EventSender,
    stripper: &mut EscapeStripper,
    bytes: &[u8],
) {
    let _ = event_tx.send(Ok(AcpEvent::Terminal {
        terminal_id: tid.to_owned(),
        bytes: bytes.to_vec(),
    }));
    let text = stripper.feed(bytes);
    if let Some(state) = lock(mgr).get_mut(tid) {
        append_output(state, &text);
    }
}

pub(crate) fn close(mgr: &TerminalManager) {
    let mut map = lock(mgr);
    for state in map.values_mut() {
        if !state.done {
            state.killed = true;
            state.cancel.cancel();
        }
        state.released = true;
    }
}

fn terminal_spec(args: &CreateTerminalRequest) -> magistr_process::ProcessSpec {
    let mut argv = Vec::with_capacity(args.args.len() + 1);
    argv.push(args.command.clone());
    argv.extend(args.args.iter().cloned());
    let mut spec = magistr_process::ProcessSpec::new(
        magistr_process::Launch::Argv(argv),
        Duration::from_secs(24 * 60 * 60),
    )
    .on_terminal(PTY_SIZE.0, PTY_SIZE.1);
    if let Some(cwd) = &args.cwd {
        spec = spec.in_dir(cwd.clone());
    }
    spec.with_env(
        args.env
            .iter()
            .map(|entry| (entry.name.clone(), entry.value.clone())),
    )
}

pub(crate) fn create_terminal(
    args: CreateTerminalRequest,
    mgr: &TerminalManager,
    event_tx: &EventSender,
) -> std::result::Result<CreateTerminalResponse, Error> {
    let terminal_id = next_terminal_id();
    let process = magistr_process::OwnedProcess::spawn(terminal_spec(&args)).map_err(|error| {
        Error::new(
            INTERNAL_ERROR,
            format!(
                "Failed to spawn terminal command '{}': {error}",
                args.command
            ),
        )
    })?;
    let pty = process.is_terminal();
    let cancel = CancellationToken::new();
    lock(mgr).insert(
        terminal_id.clone(),
        TerminalState {
            output: String::new(),
            output_byte_limit: args.output_byte_limit,
            truncated: false,
            exit_status: None,
            done: false,
            killed: false,
            released: false,
            cancel: cancel.clone(),
        },
    );
    let _ = event_tx.send(Ok(AcpEvent::System {
        subtype: "terminal_create".to_string(),
        data: serde_json::json!({
            "terminal_id": &terminal_id,
            "command": &args.command,
            "args": &args.args,
            "pty": pty,
            "pane": if pty { "live" } else { "text-only" },
            "reason": if pty { serde_json::Value::Null } else { serde_json::json!("pty unavailable") },
        }),
    }));
    let worker_mgr = Arc::clone(mgr);
    let worker_tid = terminal_id.clone();
    let worker_tx = event_tx.clone();
    tokio::spawn(async move {
        let mut stripper = EscapeStripper::default();
        let outcome = process
            .run(&cancel, |bytes| {
                ingest(&worker_mgr, &worker_tid, &worker_tx, &mut stripper, bytes)
            })
            .await;
        let mut map = lock(&worker_mgr);
        if let Some(state) = map.get_mut(&worker_tid) {
            state.exit_status = Some(TerminalExitStatus::new().exit_code(
                if state.killed || outcome.cancelled {
                    None
                } else {
                    outcome.exit_code.map(|code| code as u32)
                },
            ));
            state.done = true;
        }
    });
    Ok(CreateTerminalResponse::new(terminal_id))
}

pub(crate) fn terminal_output(
    args: &TerminalOutputRequest,
    mgr: &TerminalManager,
) -> std::result::Result<TerminalOutputResponse, Error> {
    let tid = args.terminal_id.0.to_string();
    let map = lock(mgr);
    let state = map.get(&tid).ok_or_else(|| unknown(&tid))?;
    Ok(TerminalOutputResponse::new(&state.output, state.truncated)
        .exit_status(state.exit_status.clone()))
}

pub(crate) async fn wait_for_terminal_exit(
    args: &WaitForTerminalExitRequest,
    mgr: &TerminalManager,
) -> std::result::Result<WaitForTerminalExitResponse, Error> {
    let tid = args.terminal_id.0.to_string();
    loop {
        let status = {
            let map = lock(mgr);
            let state = map.get(&tid).ok_or_else(|| unknown(&tid))?;
            state.done.then(|| {
                state
                    .exit_status
                    .clone()
                    .unwrap_or_else(TerminalExitStatus::new)
            })
        };
        if let Some(status) = status {
            return Ok(WaitForTerminalExitResponse::new(status));
        }
        tokio::time::sleep(WAIT_POLL).await;
    }
}

pub(crate) fn kill_terminal(
    args: &KillTerminalRequest,
    mgr: &TerminalManager,
) -> std::result::Result<KillTerminalResponse, Error> {
    kill(mgr, &args.terminal_id.0)?;
    Ok(KillTerminalResponse::new())
}

pub(crate) fn kill(mgr: &TerminalManager, tid: &str) -> std::result::Result<(), Error> {
    let mut map = lock(mgr);
    let state = map.get_mut(tid).ok_or_else(|| unknown(tid))?;
    if state.released {
        return Err(Error::new(
            INVALID_PARAMS,
            format!("Terminal {tid} has been released"),
        ));
    }
    if !state.done {
        state.killed = true;
        state.cancel.cancel();
    }
    Ok(())
}

pub(crate) fn release_terminal(
    args: &ReleaseTerminalRequest,
    mgr: &TerminalManager,
    event_tx: &EventSender,
) -> std::result::Result<ReleaseTerminalResponse, Error> {
    let tid = args.terminal_id.0.to_string();
    let first = {
        let mut map = lock(mgr);
        let state = map.get_mut(&tid).ok_or_else(|| unknown(&tid))?;
        if !state.done {
            state.killed = true;
            state.cancel.cancel();
        }
        let first = !state.released;
        state.released = true;
        first
    };
    if first {
        let _ = event_tx.send(Ok(AcpEvent::System {
            subtype: "terminal_release".to_string(),
            data: serde_json::json!({ "terminal_id": &tid }),
        }));
    }
    Ok(ReleaseTerminalResponse::new())
}

#[cfg(test)]
#[path = "terminal.tests.rs"]
mod tests;
