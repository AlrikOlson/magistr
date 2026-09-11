use std::{sync::Arc, time::Instant};

use super::{EngineSpan, SpanOutcome, SpanStatus, trace_recorder::TraceInner};

/// One served request, recorded as `tool:<name>`; children link to it by
/// trace id and parent span id. A span never ended emits nothing — the
/// sanctioned way to drop work with no taxonomy outcome.
pub struct ToolSpan {
    pub(super) inner: Arc<TraceInner>,
    pub(super) tool: String,
    pub(super) trace_id: String,
    pub(super) span_id: String,
    pub(super) start_unix_ms: u64,
    pub(super) started: Instant,
}

impl ToolSpan {
    #[must_use]
    pub fn trace_id(&self) -> &str {
        &self.trace_id
    }

    #[must_use]
    pub fn span_id(&self) -> &str {
        &self.span_id
    }

    /// A linked child for one engine call made while serving this request.
    #[must_use]
    pub fn engine_child(&self, method: impl Into<String>) -> EngineSpan {
        EngineSpan {
            inner: self.inner.clone(),
            trace_id: self.trace_id.clone(),
            parent_span_id: self.span_id.clone(),
            method: method.into(),
            start_unix_ms: TraceInner::now_unix_ms(),
            started: Instant::now(),
        }
    }

    pub fn end(self, status: &SpanStatus, outcome: SpanOutcome) {
        let mut attributes = serde_json::Map::new();
        attributes.insert("tool".into(), self.tool.clone().into());
        if let Some(envelope_v) = outcome.envelope_v {
            attributes.insert("envelopeV".into(), envelope_v.into());
        }
        if let Some(notes_count) = outcome.notes_count {
            attributes.insert("notesCount".into(), notes_count.into());
        }
        if let Some(truncated) = outcome.truncated {
            attributes.insert("truncated".into(), truncated.into());
        }
        let record = serde_json::json!({
            "v": TraceInner::RECORD_VERSION,
            "traceId": self.trace_id,
            "spanId": self.span_id,
            "name": format!("tool:{}", self.tool),
            "startUnixMs": self.start_unix_ms,
            "durationMs": self.started.elapsed().as_secs_f64() * 1000.0,
            "status": status.wire(),
            "attributes": attributes,
        });
        self.inner.emit(&record);
    }
}
