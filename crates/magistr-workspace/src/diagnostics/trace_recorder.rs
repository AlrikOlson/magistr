use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc,
        mpsc::{self, Receiver, Sender},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use super::ToolSpan;
use crate::response::Redactor;

/// The JSONL flight recorder: OTel-shaped span records, one JSON line each
/// in `<state>/trace.jsonl`. The ring buffer is the retained shadow-log
/// mechanism: a fixed cap, the newest half of lines kept on breach, every
/// IO error swallowed, appends fire-and-forget on a dedicated thread so the
/// served path never awaits trace IO. Privacy is structural (typed
/// attributes only) and every line still passes the credential redactor.
/// Cache-class data: losing the file loses history, never correctness.
pub struct TraceRecorder {
    inner: Arc<TraceInner>,
}

pub(super) struct TraceInner {
    sender: Option<Sender<Message>>,
    path: Option<PathBuf>,
}

enum Message {
    Line(String),
    Flush(Sender<()>),
}

impl TraceRecorder {
    /// Trace-log size cap; the newest half of lines survives a breach.
    pub const LOG_CAP_BYTES: u64 = 8 * 1024 * 1024;

    /// Record to `path`. The writer thread is started here; if it cannot be,
    /// the recorder is silently disabled — tracing never fails a request.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path: PathBuf = path.into();
        let (sender, receiver) = mpsc::channel();
        let writer_path = path.clone();
        let spawned = std::thread::Builder::new()
            .name("magistr-trace".into())
            .spawn(move || writer(&writer_path, &receiver));
        Self {
            inner: Arc::new(TraceInner {
                sender: spawned.is_ok().then_some(sender),
                path: Some(path),
            }),
        }
    }

    /// Wired when tracing is off: nothing behaves differently untraced.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            inner: Arc::new(TraceInner {
                sender: None,
                path: None,
            }),
        }
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.inner.sender.is_some()
    }

    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.inner.path.as_deref()
    }

    #[must_use]
    pub fn start_tool_span(&self, tool: impl Into<String>) -> ToolSpan {
        ToolSpan {
            inner: self.inner.clone(),
            tool: tool.into(),
            trace_id: TraceInner::trace_id(),
            span_id: TraceInner::span_id(),
            start_unix_ms: TraceInner::now_unix_ms(),
            started: Instant::now(),
        }
    }

    /// Wait for queued writes to land — a seam for tests and shutdown only;
    /// bounded so a wedged disk cannot hold a shutdown hostage.
    pub fn settle(&self) {
        let Some(sender) = &self.inner.sender else {
            return;
        };
        let (reply, done) = mpsc::channel();
        if sender.send(Message::Flush(reply)).is_ok() {
            let _ = done.recv_timeout(Duration::from_secs(5));
        }
    }
}

impl TraceInner {
    pub(super) const RECORD_VERSION: u64 = 1;

    /// Fire-and-forget: the line is redacted, then queued; failures vanish.
    pub(super) fn emit(&self, record: &serde_json::Value) {
        let Some(sender) = &self.sender else {
            return;
        };
        let Ok(line) = serde_json::to_string(record) else {
            return;
        };
        let _ = sender.send(Message::Line(Redactor::redact(&line).text));
    }

    pub(super) fn now_unix_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis() as u64)
            .unwrap_or(0)
    }

    /// OTel id widths: 16-byte trace id, 8-byte span id, lowercase hex.
    pub(super) fn trace_id() -> String {
        random_hex::<16>()
    }

    pub(super) fn span_id() -> String {
        random_hex::<8>()
    }
}

fn random_hex<const N: usize>() -> String {
    let mut bytes = [0_u8; N];
    if getrandom::fill(&mut bytes).is_err() {
        // Entropy failure degrades linkage, never the request: a timestamp
        // still keeps records distinguishable.
        let stamp = TraceInner::now_unix_ms().to_le_bytes();
        for (slot, byte) in bytes.iter_mut().zip(stamp.iter().cycle()) {
            *slot = *byte;
        }
    }
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn writer(path: &Path, receiver: &Receiver<Message>) {
    for message in receiver {
        match message {
            Message::Line(line) => write_line(path, &line),
            Message::Flush(reply) => {
                let _ = reply.send(());
            }
        }
    }
}

/// Cap eviction never races an append: this thread is the only writer.
/// The writer never repairs a torn tail (contract): a record appended
/// directly onto a tear merges into it and is lost; everything after is intact.
fn write_line(path: &Path, line: &str) {
    let size = std::fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let result = if size + line.len() as u64 + 1 > TraceRecorder::LOG_CAP_BYTES {
        let existing = std::fs::read_to_string(path).unwrap_or_default();
        let lines: Vec<&str> = existing
            .split('\n')
            .filter(|line| !line.is_empty())
            .collect();
        let mut kept: Vec<&str> = lines[lines.len().div_ceil(2)..].to_vec();
        kept.push(line);
        std::fs::write(path, format!("{}\n", kept.join("\n")))
    } else {
        std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
            .and_then(|mut file| file.write_all(format!("{line}\n").as_bytes()))
    };
    // Tracing must never fail anything.
    let _ = result;
}

#[cfg(test)]
#[path = "trace_recorder.tests.rs"]
mod tests;
