use serde::Serialize;

use crate::signal::domain::{Signal, SignalKind, SignalStatus};

/// A bounded signal row in an inbox snapshot.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SignalSummary {
    pub id: String,
    pub kind: SignalKind,
    pub status: SignalStatus,
    pub from: String,
    pub body: String,
    pub created: String,
}

impl From<&Signal> for SignalSummary {
    fn from(signal: &Signal) -> Self {
        const BODY_LEN: usize = 140;
        let body = if signal.body.chars().count() <= BODY_LEN {
            signal.body.clone()
        } else {
            let cut: String = signal.body.chars().take(BODY_LEN).collect();
            format!("{}…", cut.trim_end())
        };
        Self {
            id: signal.id.clone(),
            kind: signal.kind,
            status: signal.status,
            from: signal.from.clone(),
            body,
            created: signal.created.clone(),
        }
    }
}
