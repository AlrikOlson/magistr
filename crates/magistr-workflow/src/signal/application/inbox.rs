use std::collections::BTreeMap;

use serde::Serialize;

use super::SignalSummary;
use crate::signal::domain::{SignalStatus, Signals};

/// Counts and bounded recent records for the native signal inbox.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SignalInbox {
    pub project_id: String,
    pub counts: BTreeMap<&'static str, usize>,
    pub signals: Vec<SignalSummary>,
    pub omitted: usize,
}

impl From<&Signals> for SignalInbox {
    fn from(store: &Signals) -> Self {
        const LIST_CAP: usize = 60;
        let counts = [
            ("new", SignalStatus::New),
            ("triaged", SignalStatus::Triaged),
            ("researched", SignalStatus::Researched),
            ("surfaced", SignalStatus::Surfaced),
            ("promoted", SignalStatus::Promoted),
            ("dismissed", SignalStatus::Dismissed),
        ]
        .into_iter()
        .map(|(name, status)| {
            (
                name,
                store.signals.iter().filter(|s| s.status == status).count(),
            )
        })
        .chain([("total", store.signals.len())])
        .collect();
        let mut recent: Vec<_> = store.signals.iter().collect();
        recent.sort_by(|a, b| b.created.cmp(&a.created));
        Self {
            project_id: store.project_id.clone(),
            counts,
            signals: recent
                .into_iter()
                .take(LIST_CAP)
                .map(SignalSummary::from)
                .collect(),
            omitted: store.signals.len().saturating_sub(LIST_CAP),
        }
    }
}
