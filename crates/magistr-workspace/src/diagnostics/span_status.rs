use crate::contracts::ErrorKind;

/// A span's outcome: ok, or a taxonomy code whose recovery class the
/// recorder derives — a code paired with the wrong recovery cannot exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpanStatus {
    Ok,
    Failed(ErrorKind),
}

impl SpanStatus {
    pub(super) fn wire(&self) -> serde_json::Value {
        match self {
            Self::Ok => serde_json::json!({ "ok": true }),
            Self::Failed(kind) => serde_json::json!({
                "ok": false,
                "code": kind.code(),
                "recovery": kind.recovery().as_str(),
            }),
        }
    }
}
