use std::{sync::Arc, time::Instant};

use super::{SpanStatus, trace_recorder::TraceInner};

/// One engine or worker call made while serving a request, recorded as
/// `engine:<method>` under its root. A span never ended emits nothing.
pub struct EngineSpan {
    pub(super) inner: Arc<TraceInner>,
    pub(super) trace_id: String,
    pub(super) parent_span_id: String,
    pub(super) method: String,
    pub(super) start_unix_ms: u64,
    pub(super) started: Instant,
}

impl EngineSpan {
    pub fn end(self, status: &SpanStatus) {
        let record = serde_json::json!({
            "v": TraceInner::RECORD_VERSION,
            "traceId": self.trace_id,
            "spanId": TraceInner::span_id(),
            "parentSpanId": self.parent_span_id,
            "name": format!("engine:{}", self.method),
            "startUnixMs": self.start_unix_ms,
            "durationMs": self.started.elapsed().as_secs_f64() * 1000.0,
            "status": status.wire(),
            "attributes": { "method": self.method },
        });
        self.inner.emit(&record);
    }
}
